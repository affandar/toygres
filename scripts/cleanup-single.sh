#!/bin/bash
set -e

# Check if instance name is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <instance-name>"
    echo ""
    echo "Example:"
    echo "  $0 test-pg-1"
    exit 1
fi

INSTANCE_NAME=$1

# Configuration
NAMESPACE=${AKS_NAMESPACE:-toygres}

# Load environment variables from .env if exists (but don't override existing vars)
if [ -f .env ] && [ -z "$AKS_NAMESPACE" ]; then
    export $(cat .env | grep -v '^#' | xargs)
    NAMESPACE=${AKS_NAMESPACE:-toygres}
fi

echo "=== Cleaning up PostgreSQL Instance ==="
echo ""
echo "Instance: $INSTANCE_NAME"
echo "Namespace: $NAMESPACE"
echo ""

# Check if namespace exists
if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
    echo "Error: Namespace '$NAMESPACE' does not exist"
    exit 1
fi

# Check if StatefulSet exists
if ! kubectl get statefulset -n "$NAMESPACE" "$INSTANCE_NAME" &> /dev/null; then
    echo "⚠ Warning: StatefulSet '$INSTANCE_NAME' not found"
else
    echo "Found deployment: $INSTANCE_NAME"
fi

echo ""
read -p "Delete this deployment? (y/n): " CONFIRM
if [ "$CONFIRM" != "y" ]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo "Deleting resources..."
echo ""

# Delete StatefulSet
echo "Deleting StatefulSet..."
if kubectl delete statefulset -n "$NAMESPACE" "$INSTANCE_NAME" 2>/dev/null; then
    echo "  ✓ StatefulSet deleted"
else
    echo "  ✗ Failed to delete StatefulSet (may not exist)"
fi

# Delete Service
echo "Deleting Service..."
if kubectl delete service -n "$NAMESPACE" "${INSTANCE_NAME}-svc" 2>/dev/null; then
    echo "  ✓ Service deleted"
else
    echo "  ✗ Failed to delete Service (may not exist)"
fi

# Wait a bit for pods to terminate
echo "Waiting for pods to terminate..."
sleep 5

# Delete PVC
echo "Deleting PVC..."
if kubectl delete pvc -n "$NAMESPACE" "${INSTANCE_NAME}-pvc" 2>/dev/null; then
    echo "  ✓ PVC deleted"
else
    echo "  ✗ Failed to delete PVC (may not exist)"
fi

echo ""
echo "✓ Cleanup complete for $INSTANCE_NAME"

