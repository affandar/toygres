#!/bin/bash
set -e

echo "Toygres Infrastructure Setup"
echo "============================"
echo ""
echo "This script helps you set up the foundational Azure infrastructure for Toygres."
echo ""

# Check if Azure CLI is installed
if ! command -v az &> /dev/null; then
    echo "Error: Azure CLI is not installed. Please install it first:"
    echo "  https://docs.microsoft.com/en-us/cli/azure/install-azure-cli"
    exit 1
fi

# Check if logged in
if ! az account show &> /dev/null; then
    echo "Error: Not logged in to Azure. Please run: az login"
    exit 1
fi

# Configuration
read -p "Enter Azure Resource Group name (default: toygres-rg): " RESOURCE_GROUP
RESOURCE_GROUP=${RESOURCE_GROUP:-toygres-rg}

read -p "Enter Azure Region (default: eastus): " LOCATION
LOCATION=${LOCATION:-eastus}

read -p "Enter AKS Cluster name (default: toygres-aks): " CLUSTER_NAME
CLUSTER_NAME=${CLUSTER_NAME:-toygres-aks}

read -p "Enter AKS Node Count (default: 3): " NODE_COUNT
NODE_COUNT=${NODE_COUNT:-3}

read -p "Enter AKS Node VM Size (default: Standard_D2s_v3): " NODE_SIZE
NODE_SIZE=${NODE_SIZE:-Standard_D2s_v3}

echo ""
echo "Configuration:"
echo "  Resource Group: $RESOURCE_GROUP"
echo "  Location: $LOCATION"
echo "  AKS Cluster: $CLUSTER_NAME"
echo "  Node Count: $NODE_COUNT"
echo "  Node Size: $NODE_SIZE"
echo ""

read -p "Proceed with creation? (y/n): " CONFIRM
if [ "$CONFIRM" != "y" ]; then
    echo "Aborted."
    exit 0
fi

echo ""
echo "Creating resource group..."
az group create --name "$RESOURCE_GROUP" --location "$LOCATION"

echo ""
echo "Creating AKS cluster (this may take 10-15 minutes)..."
az aks create \
    --resource-group "$RESOURCE_GROUP" \
    --name "$CLUSTER_NAME" \
    --node-count "$NODE_COUNT" \
    --node-vm-size "$NODE_SIZE" \
    --enable-managed-identity \
    --generate-ssh-keys \
    --network-plugin azure \
    --network-policy azure

echo ""
echo "Getting AKS credentials..."
az aks get-credentials --resource-group "$RESOURCE_GROUP" --name "$CLUSTER_NAME" --overwrite-existing

echo ""
echo "Creating toygres namespace..."
kubectl create namespace toygres --dry-run=client -o yaml | kubectl apply -f -

echo ""
echo "Creating storage class for PostgreSQL..."
kubectl apply -f - <<EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: toygres-postgres-storage
provisioner: kubernetes.io/azure-disk
parameters:
  storageaccounttype: Premium_LRS
  kind: Managed
reclaimPolicy: Delete
volumeBindingMode: WaitForFirstConsumer
allowVolumeExpansion: true
EOF

echo ""
echo "=========================================="
echo "Infrastructure setup complete!"
echo "=========================================="
echo ""
echo "Add these to your .env file:"
echo ""
echo "AKS_CLUSTER_NAME=$CLUSTER_NAME"
echo "AKS_RESOURCE_GROUP=$RESOURCE_GROUP"
echo "AKS_NAMESPACE=toygres"
echo ""
echo "Your kubectl is now configured to use this cluster."
echo "You can verify with: kubectl cluster-info"

