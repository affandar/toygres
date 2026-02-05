# Push PostgreSQL Container Image to Toygres

Help me push a PostgreSQL container image to the Toygres ACR and register it for deployment.

## My Image Source

**Source:** [FILL IN - e.g., `docker.io/postgres:16`, `/path/to/image.tar`, or `local:myimage:tag`]

**Name for this image in Toygres:** [FILL IN - e.g., `postgres-16-custom`]

**Description (optional):** [FILL IN]

---

## Instructions for the AI

1. **Determine the image source type:**
   - Remote registry (e.g., `docker.io/...`, `gcr.io/...`, `mcr.microsoft.com/...`)
   - Local tar file (`.tar` or `.tar.gz`)
   - Local Docker image (prefix with `local:`)

2. **Pull/load the image locally** using Docker CLI

3. **Login to Toygres ACR:**
   - Get ACR name from `$TOYGRES_ACR_NAME` in `.env`, or ask the user if not set
   ```bash
   az acr login --name $TOYGRES_ACR_NAME
   ```

4. **Tag and push to ACR:**
   ```bash
   docker tag <source> $TOYGRES_ACR_NAME.azurecr.io/<name>:<timestamp>
   docker push $TOYGRES_ACR_NAME.azurecr.io/<name>:<timestamp>
   ```

5. **Get the image digest from ACR:**
   ```bash
   az acr repository show-manifests --name $TOYGRES_ACR_NAME --repository <name> --orderby time_desc --top 1 --query "[0].digest" -o tsv
   ```

6. **Authenticate with Toygres API:**
   - API URL: `$TOYGRES_API_URL` from `.env` (defaults to `http://localhost:8080` if not set)
   - Credentials: `$TOYGRES_ADMIN_USERNAME` and `$TOYGRES_ADMIN_PASSWORD` from `.env`
   - **IMPORTANT:** If `.env` doesn't exist or doesn't contain required variables, ask the user to create/update the `.env` file. Do NOT ask the user to paste credentials in chat.
   - **NEVER print or echo credential values in chat or terminal output.**
   - Login via POST to `/login` with `username` and `password` form fields
   - Save the session cookie for subsequent requests

7. **Register the PostgreSQL image via API:**
   ```bash
   curl -b <cookies> -X POST $TOYGRES_API_URL/api/runtime-images/register \
     -H "Content-Type: application/json" \
     -d '{"name": "<name>", "acr_ref": "$TOYGRES_ACR_NAME.azurecr.io/<name>", "digest": "<digest>", "description": "<desc>"}'
   ```

8. **Confirm success** and provide the pull reference: `$TOYGRES_ACR_NAME.azurecr.io/<name>@<digest>`

---

## Prerequisites

The user must have:
- Azure CLI (`az`) logged in with ACR push permissions
- Docker CLI
- A `.env` file in the project root containing:
  - `TOYGRES_ACR_NAME` - Azure Container Registry name (e.g., `myacr`)
  - `TOYGRES_API_URL` - API endpoint (optional, defaults to `http://localhost:8080`)
  - `TOYGRES_ADMIN_USERNAME` and `TOYGRES_ADMIN_PASSWORD` - Admin credentials
  - If any required variable is missing, ask the user to update `.env`
  - **NEVER ask for credentials in chat or print them in terminal output**
- Network access to the Toygres API

---

## Notes

- PostgreSQL images uploaded this way are always deployed in **stock** mode (pg_durable is a built-in special mode, not uploadable)
- The digest is pinned for reproducibility - the same image will always be deployed
- After registration, the image appears in the Toygres UI under "Runtime Images" and can be selected when creating instances
