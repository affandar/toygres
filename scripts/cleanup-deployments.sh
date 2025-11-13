#!/bin/bash
set -e

echo "=== Toygres Deployment Cleanup ==="
echo ""

# Configuration
NAMESPACE=${AKS_NAMESPACE:-toygres}

# Load environment variables from .env if exists (but don't override existing vars)
if [ -f .env ] && [ -z "$AKS_NAMESPACE" ]; then
    export $(cat .env | grep -v '^#' | xargs)
    NAMESPACE=${AKS_NAMESPACE:-toygres}
fi

echo "Configuration:"
echo "  Namespace: $NAMESPACE"
echo ""

# Check if namespace exists
if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
    echo "Error: Namespace '$NAMESPACE' does not exist"
    exit 1
fi

echo "Finding all Toygres PostgreSQL deployments..."
echo ""

# Find all StatefulSets with app=postgres label
STATEFULSETS=$(kubectl get statefulset -n "$NAMESPACE" -l app=postgres -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")

if [ -z "$STATEFULSETS" ]; then
    echo "✓ No deployments found in namespace '$NAMESPACE'"
    exit 0
fi

echo "Found the following deployments:"
for sts in $STATEFULSETS; do
    echo "  - $sts"
done
echo ""

# Ask for confirmation
read -p "Delete all these deployments? (y/n): " CONFIRM
if [ "$CONFIRM" != "y" ]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo "Cleaning up deployments..."
echo ""

# Delete each deployment
for sts in $STATEFULSETS; do
    echo "--- Cleaning up: $sts ---"
    
    # Delete StatefulSet
    echo "  Deleting StatefulSet..."
    if kubectl delete statefulset -n "$NAMESPACE" "$sts" 2>/dev/null; then
        echo "    ✓ StatefulSet deleted"
    else
        echo "    ✗ Failed to delete StatefulSet (may not exist)"
    fi
    
    # Delete Service
    echo "  Deleting Service..."
    if kubectl delete service -n "$NAMESPACE" "${sts}-svc" 2>/dev/null; then
        echo "    ✓ Service deleted"
    else
        echo "    ✗ Failed to delete Service (may not exist)"
    fi
    
    # Delete PVC
    echo "  Deleting PVC..."
    if kubectl delete pvc -n "$NAMESPACE" "${sts}-pvc" 2>/dev/null; then
        echo "    ✓ PVC deleted"
    else
        echo "    ✗ Failed to delete PVC (may not exist)"
    fi
    
    echo ""
done

echo "=== Cleanup Complete ==="
echo ""
echo "Verifying..."
REMAINING=$(kubectl get statefulset -n "$NAMESPACE" -l app=postgres -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")
if [ -z "$REMAINING" ]; then
    echo "✓ All deployments cleaned up successfully"
else
    echo "⚠ Some deployments still remain: $REMAINING"
fi

