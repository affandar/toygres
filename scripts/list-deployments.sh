#!/bin/bash
set -e

echo "=== Toygres PostgreSQL Deployments ==="
echo ""

# Configuration
NAMESPACE=${AKS_NAMESPACE:-toygres}

# Load environment variables from .env if exists (but don't override existing vars)
if [ -f .env ] && [ -z "$AKS_NAMESPACE" ]; then
    export $(cat .env | grep -v '^#' | xargs)
    NAMESPACE=${AKS_NAMESPACE:-toygres}
fi

echo "Namespace: $NAMESPACE"
echo ""

# Check if namespace exists
if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
    echo "Error: Namespace '$NAMESPACE' does not exist"
    exit 1
fi

# List all StatefulSets with app=postgres label
echo "StatefulSets:"
if kubectl get statefulset -n "$NAMESPACE" -l app=postgres 2>/dev/null | grep -q "No resources found"; then
    echo "  No PostgreSQL StatefulSets found"
else
    kubectl get statefulset -n "$NAMESPACE" -l app=postgres -o wide 2>/dev/null || echo "  No PostgreSQL StatefulSets found"
fi
echo ""

# List all Services with app=postgres label
echo "Services:"
if kubectl get service -n "$NAMESPACE" -l app=postgres 2>/dev/null | grep -q "No resources found"; then
    echo "  No PostgreSQL Services found"
else
    kubectl get service -n "$NAMESPACE" -l app=postgres -o wide 2>/dev/null || echo "  No PostgreSQL Services found"
fi
echo ""

# List all PVCs with app=postgres label
echo "PersistentVolumeClaims:"
if kubectl get pvc -n "$NAMESPACE" -l app=postgres 2>/dev/null | grep -q "No resources found"; then
    echo "  No PostgreSQL PVCs found"
else
    kubectl get pvc -n "$NAMESPACE" -l app=postgres 2>/dev/null || echo "  No PostgreSQL PVCs found"
fi
echo ""

# List all Pods with app=postgres label
echo "Pods:"
if kubectl get pods -n "$NAMESPACE" -l app=postgres 2>/dev/null | grep -q "No resources found"; then
    echo "  No PostgreSQL Pods found"
else
    kubectl get pods -n "$NAMESPACE" -l app=postgres -o wide 2>/dev/null || echo "  No PostgreSQL Pods found"
fi

