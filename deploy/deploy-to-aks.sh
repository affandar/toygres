#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "${BLUE}🚀 Toygres AKS Deployment${NC}"
echo "========================================"

# Parse arguments
ENABLE_HTTPS=false
TOYGRES_DNS_LABEL=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --https)
            ENABLE_HTTPS=true
            shift
            ;;
        --dns-label)
            TOYGRES_DNS_LABEL="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Usage: $0 [--https] [--dns-label <label>]"
            exit 1
            ;;
    esac
done

# Load environment variables
if [ -f "$PROJECT_ROOT/.env" ]; then
    echo -e "${BLUE}📋 Loading environment variables...${NC}"
    set -a
    source "$PROJECT_ROOT/.env"
    set +a
else
    echo -e "${RED}❌ .env file not found at $PROJECT_ROOT/.env${NC}"
    exit 1
fi

# Validate required environment variables
REQUIRED_VARS="AKS_CLUSTER_NAME AKS_RESOURCE_GROUP DATABASE_URL TOYGRES_ADMIN_USERNAME TOYGRES_ADMIN_PASSWORD AZURE_CLIENT_ID AZURE_CLIENT_SECRET AZURE_TENANT_ID"
for var in $REQUIRED_VARS; do
    if [ -z "${!var}" ]; then
        echo -e "${RED}❌ Missing required environment variable: $var${NC}"
        exit 1
    fi
done

# Set ACR name (derived from cluster name)
ACR_NAME="${AKS_CLUSTER_NAME}acr"
ACR_NAME=$(echo "$ACR_NAME" | tr '[:upper:]' '[:lower:]' | tr -d '-')

# Get Azure region from AKS cluster
AZURE_REGION=$(az aks show --resource-group "$AKS_RESOURCE_GROUP" --name "$AKS_CLUSTER_NAME" --query location -o tsv 2>/dev/null || echo "westus3")

echo -e "${GREEN}✓ Environment validated${NC}"
echo "  Cluster: $AKS_CLUSTER_NAME"
echo "  Resource Group: $AKS_RESOURCE_GROUP"
echo "  ACR: $ACR_NAME"
echo "  HTTPS: $ENABLE_HTTPS"

# Login to Azure
echo -e "\n${BLUE}🔐 Logging into Azure...${NC}"
az login --service-principal \
    --username "$AZURE_CLIENT_ID" \
    --password "$AZURE_CLIENT_SECRET" \
    --tenant "$AZURE_TENANT_ID" \
    --output none

echo -e "${GREEN}✓ Azure login successful${NC}"

# Create ACR if it doesn't exist
echo -e "\n${BLUE}📦 Setting up Azure Container Registry...${NC}"
if ! az acr show --name "$ACR_NAME" --resource-group "$AKS_RESOURCE_GROUP" &>/dev/null; then
    echo "  Creating ACR: $ACR_NAME..."
    az acr create \
        --resource-group "$AKS_RESOURCE_GROUP" \
        --name "$ACR_NAME" \
        --sku Basic \
        --output none
    echo -e "${GREEN}✓ ACR created${NC}"
else
    echo -e "${GREEN}✓ ACR already exists${NC}"
fi

# Login to ACR
echo "  Logging into ACR..."
az acr login --name "$ACR_NAME"

# Attach ACR to AKS if not already attached
echo "  Attaching ACR to AKS cluster..."
az aks update \
    --resource-group "$AKS_RESOURCE_GROUP" \
    --name "$AKS_CLUSTER_NAME" \
    --attach-acr "$ACR_NAME" \
    --output none 2>/dev/null || true

echo -e "${GREEN}✓ ACR setup complete${NC}"

# Build and push Docker images
echo -e "\n${BLUE}🐳 Building Docker images...${NC}"

cd "$PROJECT_ROOT"

# Build UI first (needs npm build)
echo "  Building toygres-ui (frontend)..."
cd toygres-ui
npm ci --silent
npm run build --silent
cd "$PROJECT_ROOT"

# Build server image (cross-platform for AKS)
echo "  Building toygres-server..."
DOCKER_BUILDKIT=1 docker build --platform linux/amd64 -f deploy/Dockerfile.server -t "$ACR_NAME.azurecr.io/toygres-server:latest" .
docker push "$ACR_NAME.azurecr.io/toygres-server:latest"
echo -e "${GREEN}✓ toygres-server pushed${NC}"

# Build UI image
echo "  Building toygres-ui..."
DOCKER_BUILDKIT=1 docker build --platform linux/amd64 -f deploy/Dockerfile.ui -t "$ACR_NAME.azurecr.io/toygres-ui:latest" .
docker push "$ACR_NAME.azurecr.io/toygres-ui:latest"
echo -e "${GREEN}✓ toygres-ui pushed${NC}"

# Get AKS credentials
echo -e "\n${BLUE}🔑 Getting AKS credentials...${NC}"
az aks get-credentials \
    --resource-group "$AKS_RESOURCE_GROUP" \
    --name "$AKS_CLUSTER_NAME" \
    --overwrite-existing

echo -e "${GREEN}✓ kubectl configured${NC}"

# Create namespace
echo -e "\n${BLUE}📁 Creating namespace...${NC}"
kubectl apply -f "$SCRIPT_DIR/k8s/namespace.yaml"

# Create secrets from environment variables
echo -e "\n${BLUE}🔒 Creating secrets...${NC}"
kubectl create secret generic toygres-secrets \
    --namespace toygres-system \
    --from-literal=DATABASE_URL="$DATABASE_URL" \
    --from-literal=TOYGRES_ADMIN_USERNAME="$TOYGRES_ADMIN_USERNAME" \
    --from-literal=TOYGRES_ADMIN_PASSWORD="$TOYGRES_ADMIN_PASSWORD" \
    --from-literal=AZURE_CLIENT_ID="$AZURE_CLIENT_ID" \
    --from-literal=AZURE_CLIENT_SECRET="$AZURE_CLIENT_SECRET" \
    --from-literal=AZURE_TENANT_ID="$AZURE_TENANT_ID" \
    --dry-run=client -o yaml | kubectl apply -f -

echo -e "${GREEN}✓ Secrets created${NC}"

# Apply Kubernetes manifests
echo -e "\n${BLUE}📦 Deploying to Kubernetes...${NC}"

# Replace ACR_NAME in manifests and apply (skip secret.yaml and ingress.yaml)
# Also get the storage identity client ID for workload identity
AZURE_STORAGE_CLIENT_ID=$(az identity show --name toygres-storage-identity --resource-group "$AKS_RESOURCE_GROUP" --query clientId -o tsv 2>/dev/null || echo "")
if [ -z "$AZURE_STORAGE_CLIENT_ID" ]; then
    echo -e "${YELLOW}⚠ Storage identity not found. Image feature will not work until created.${NC}"
    AZURE_STORAGE_CLIENT_ID="placeholder-storage-client-id"
fi

for file in "$SCRIPT_DIR/k8s/"*.yaml; do
    if [ -f "$file" ]; then
        filename=$(basename "$file")
        # Skip the secret template - we already created secrets from env vars
        if [ "$filename" = "secret.yaml" ]; then
            echo "  Skipping $filename (secrets already created from env vars)"
            continue
        fi
        # Skip ingress - handled separately based on --https flag
        if [ "$filename" = "ingress.yaml" ]; then
            echo "  Skipping $filename (handled separately)"
            continue
        fi
        echo "  Applying $filename..."
        sed -e "s/\${ACR_NAME}/$ACR_NAME/g" -e "s/\${AZURE_STORAGE_CLIENT_ID}/$AZURE_STORAGE_CLIENT_ID/g" "$file" | kubectl apply -f -
    fi
done

echo -e "${GREEN}✓ Kubernetes resources deployed${NC}"

# Force restart deployments to pick up latest images
echo -e "\n${BLUE}🔄 Restarting deployments to pick up latest images...${NC}"
kubectl rollout restart deployment/toygres-server -n toygres-system
kubectl rollout restart deployment/toygres-ui -n toygres-system

# Wait for deployments
echo -e "\n${BLUE}⏳ Waiting for deployments to be ready...${NC}"
kubectl rollout status deployment/toygres-server -n toygres-system --timeout=300s
kubectl rollout status deployment/toygres-ui -n toygres-system --timeout=300s

echo -e "${GREEN}✓ Deployments ready${NC}"

# Handle HTTPS setup if requested
if [ "$ENABLE_HTTPS" = true ]; then
    echo -e "\n${BLUE}🔐 Setting up HTTPS with nginx-ingress and cert-manager...${NC}"
    
    # Check if helm is installed
    if ! command -v helm &> /dev/null; then
        echo -e "${YELLOW}  Installing Helm...${NC}"
        curl -fsSL -o get_helm.sh https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3
        chmod 700 get_helm.sh
        ./get_helm.sh
        rm get_helm.sh
    fi
    
    # Add helm repos
    helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx 2>/dev/null || true
    helm repo add jetstack https://charts.jetstack.io 2>/dev/null || true
    helm repo update
    
    # Set DNS label (use provided or default to 'toygres-app')
    if [ -z "$TOYGRES_DNS_LABEL" ]; then
        TOYGRES_DNS_LABEL="toygres-app"
    fi
    
    # Install nginx-ingress with DNS label
    echo "  Installing nginx-ingress controller..."
    helm upgrade --install ingress-nginx ingress-nginx/ingress-nginx \
        --namespace ingress-nginx \
        --create-namespace \
        --set controller.service.annotations."service\.beta\.kubernetes\.io/azure-dns-label-name"="$TOYGRES_DNS_LABEL" \
        --wait
    
    # Install cert-manager
    echo "  Installing cert-manager..."
    helm upgrade --install cert-manager jetstack/cert-manager \
        --namespace cert-manager \
        --create-namespace \
        --version v1.14.0 \
        --set installCRDs=true \
        --wait
    
    # Wait for cert-manager to be ready
    echo "  Waiting for cert-manager webhook..."
    kubectl wait --for=condition=Available deployment/cert-manager-webhook -n cert-manager --timeout=120s
    
    # Create ClusterIssuer for Let's Encrypt
    echo "  Creating Let's Encrypt ClusterIssuer..."
    cat <<EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    server: https://acme-v02.api.letsencrypt.org/directory
    email: admin@toygres.io
    privateKeySecretRef:
      name: letsencrypt-prod
    solvers:
    - http01:
        ingress:
          class: nginx
EOF
    
    # Change UI service to ClusterIP for ingress
    kubectl patch svc toygres-ui -n toygres-system -p '{"spec": {"type": "ClusterIP"}}'
    
    # Get DNS name
    TOYGRES_DNS_NAME="${TOYGRES_DNS_LABEL}.${AZURE_REGION}.cloudapp.azure.com"
    
    # Apply ingress with DNS name
    echo "  Creating Ingress resource..."
    sed "s/\${TOYGRES_DNS_NAME}/$TOYGRES_DNS_NAME/g" "$SCRIPT_DIR/k8s/ingress.yaml" | kubectl apply -f -
    
    # Update existing ingress if hostname doesn't match
    CURRENT_HOST=$(kubectl get ingress toygres-ingress -n toygres-system -o jsonpath='{.spec.rules[0].host}' 2>/dev/null || echo "")
    if [ -n "$CURRENT_HOST" ] && [ "$CURRENT_HOST" != "$TOYGRES_DNS_NAME" ]; then
        echo "  Updating ingress hostname from $CURRENT_HOST to $TOYGRES_DNS_NAME..."
        kubectl patch ingress toygres-ingress -n toygres-system --type='json' -p="[
          {\"op\": \"replace\", \"path\": \"/spec/rules/0/host\", \"value\": \"$TOYGRES_DNS_NAME\"},
          {\"op\": \"replace\", \"path\": \"/spec/tls/0/hosts/0\", \"value\": \"$TOYGRES_DNS_NAME\"}
        ]"
    fi
    
    echo -e "${GREEN}✓ HTTPS setup complete${NC}"
    echo ""
    echo -e "${BLUE}🌐 Your application will be available at:${NC}"
    echo -e "   ${GREEN}https://$TOYGRES_DNS_NAME${NC}"
    echo ""
    echo -e "${YELLOW}Note: SSL certificate may take 2-5 minutes to be issued.${NC}"
    echo "  Check status: kubectl get certificate -n toygres-system"
    echo ""
    echo -e "${YELLOW}If you can't access the site, flush your local DNS cache:${NC}"
    echo "  macOS: sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder"
    echo "  Linux: sudo systemd-resolve --flush-caches"
else
    # Get external IP for HTTP-only setup
    echo -e "\n${BLUE}🌐 Getting external IP...${NC}"
    echo "  Waiting for LoadBalancer IP (this may take a minute)..."
    
    for i in {1..30}; do
        EXTERNAL_IP=$(kubectl get svc toygres-ui -n toygres-system -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null)
        if [ -n "$EXTERNAL_IP" ]; then
            break
        fi
        sleep 5
    done
    
    if [ -n "$EXTERNAL_IP" ]; then
        echo -e "${GREEN}✓ External IP: $EXTERNAL_IP${NC}"
    else
        echo -e "${YELLOW}⚠ External IP not yet assigned. Check with:${NC}"
        echo "  kubectl get svc toygres-ui -n toygres-system"
    fi
    
    echo -e "\n${GREEN}✅ Deployment complete!${NC}"
    echo "========================================"
    echo -e "${BLUE}📊 Services:${NC}"
    if [ -n "$EXTERNAL_IP" ]; then
        echo "  Web UI: http://$EXTERNAL_IP"
    fi
fi

echo ""
echo -e "${BLUE}📋 Useful commands:${NC}"
echo "  kubectl get pods -n toygres-system"
echo "  kubectl get svc -n toygres-system"
echo "  kubectl logs -n toygres-system -l app.kubernetes.io/component=server -f"
echo "  kubectl logs -n toygres-system -l app.kubernetes.io/component=ui -f"
