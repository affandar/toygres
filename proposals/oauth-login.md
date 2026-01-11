# Proposal: OAuth Login (Google & Microsoft)

## Overview

Replace the current username/password auth with OAuth 2.0 / OpenID Connect, supporting Google and Microsoft identity providers.

## Current State

```rust
// auth.rs - Static credentials, hardcoded session token
const SESSION_TOKEN: &str = "authenticated_toygres_admin_session";

if form.username == get_admin_username() && form.password == get_admin_password() {
    cookies.add(Cookie::new(SESSION_COOKIE, SESSION_TOKEN));
}
```

**Problems:** Single user, static token, no identity, credentials in env vars.

## Proposed Architecture

```
┌─────────────┐      ┌─────────────┐      ┌──────────────────┐
│   Browser   │─────▶│  Toygres    │─────▶│ Google/Microsoft │
│             │◀─────│  Server     │◀─────│ OAuth Provider   │
└─────────────┘      └─────────────┘      └──────────────────┘
                            │
                            ▼
                     ┌─────────────┐
                     │  CMS DB     │
                     │  (sessions, │
                     │   users)    │
                     └─────────────┘
```

## OAuth Flow

```
1. User clicks "Sign in with Google"
2. Redirect to: https://accounts.google.com/o/oauth2/v2/auth
   ?client_id=xxx
   &redirect_uri=https://toygres.example.com/auth/callback
   &scope=openid email profile
   &response_type=code
   &state={random_csrf_token}

3. User authenticates with Google

4. Google redirects to: /auth/callback?code=xxx&state=xxx

5. Server exchanges code for tokens:
   POST https://oauth2.googleapis.com/token
   → Returns: { access_token, id_token, refresh_token }

6. Server decodes id_token (JWT) to get user info:
   { sub: "google-uid", email: "user@gmail.com", name: "User Name" }

7. Server creates/updates user in CMS, creates session

8. Redirect to / with session cookie
```

## Database Schema

```sql
-- migrations/cms/0007_add_oauth_users.sql

CREATE TABLE cms.users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,              -- 'google' | 'microsoft'
    provider_user_id TEXT NOT NULL,      -- sub claim from JWT
    email TEXT NOT NULL,
    name TEXT,
    avatar_url TEXT,
    role TEXT NOT NULL DEFAULT 'user',   -- 'admin' | 'user'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_login_at TIMESTAMPTZ,
    UNIQUE(provider, provider_user_id)
);

CREATE TABLE cms.sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES cms.users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,     -- SHA256 of session token
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_sessions_token ON cms.sessions(token_hash);
CREATE INDEX idx_sessions_expires ON cms.sessions(expires_at);
```

## API Changes

### New Endpoints

```
GET  /auth/providers          → List enabled providers
GET  /auth/login/google       → Redirect to Google OAuth
GET  /auth/login/microsoft    → Redirect to Microsoft OAuth
GET  /auth/callback           → OAuth callback handler
POST /auth/logout             → Clear session
GET  /api/me                  → Current user info
```

### Auth Middleware Update

```rust
// auth.rs

pub async fn auth_middleware(
    State(state): State<AppState>,
    cookies: Cookies,
    req: Request,
    next: Next,
) -> Response {
    // Public routes
    if is_public_route(req.uri().path()) {
        return next.run(req).await;
    }

    // Check session cookie
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        let token_hash = sha256(cookie.value());

        if let Some(session) = get_valid_session(&state.pool, &token_hash).await {
            // Inject user into request extensions
            let user = get_user(&state.pool, session.user_id).await;
            req.extensions_mut().insert(user);
            return next.run(req).await;
        }
    }

    // Unauthorized
    Redirect::to("/login").into_response()
}
```

## Configuration

```bash
# .env additions

# Google OAuth
GOOGLE_CLIENT_ID=xxx.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=xxx

# Microsoft OAuth (Azure AD)
MICROSOFT_CLIENT_ID=xxx
MICROSOFT_CLIENT_SECRET=xxx
MICROSOFT_TENANT_ID=common  # or specific tenant

# Session config
SESSION_SECRET=random-32-byte-hex  # For signing
SESSION_EXPIRY_HOURS=168           # 7 days

# Optional: Restrict to specific domains
ALLOWED_EMAIL_DOMAINS=company.com,partner.com
```

## Implementation

### Dependencies

```toml
# Cargo.toml
[dependencies]
oauth2 = "4.4"
jsonwebtoken = "9"
sha2 = "0.10"
```

### OAuth Client Setup

```rust
// oauth.rs

use oauth2::{
    AuthorizationCode, AuthUrl, ClientId, ClientSecret,
    CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::BasicClient,
};

pub fn google_client() -> BasicClient {
    BasicClient::new(
        ClientId::new(env::var("GOOGLE_CLIENT_ID").unwrap()),
        Some(ClientSecret::new(env::var("GOOGLE_CLIENT_SECRET").unwrap())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".into()).unwrap(),
        Some(TokenUrl::new("https://oauth2.googleapis.com/token".into()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(format!("{}/auth/callback", base_url())).unwrap())
}

pub fn microsoft_client() -> BasicClient {
    let tenant = env::var("MICROSOFT_TENANT_ID").unwrap_or("common".into());
    BasicClient::new(
        ClientId::new(env::var("MICROSOFT_CLIENT_ID").unwrap()),
        Some(ClientSecret::new(env::var("MICROSOFT_CLIENT_SECRET").unwrap())),
        AuthUrl::new(format!("https://login.microsoftonline.com/{}/oauth2/v2/authorize", tenant)).unwrap(),
        Some(TokenUrl::new(format!("https://login.microsoftonline.com/{}/oauth2/v2/token", tenant)).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(format!("{}/auth/callback", base_url())).unwrap())
}
```

### Login Handler

```rust
// routes/auth.rs

pub async fn login_google(State(state): State<AppState>) -> impl IntoResponse {
    let client = google_client();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".into()))
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .url();

    // Store CSRF token in cookie for verification
    let mut csrf_cookie = Cookie::new("oauth_csrf", csrf_token.secret().clone());
    csrf_cookie.set_http_only(true);
    csrf_cookie.set_max_age(time::Duration::minutes(10));

    (
        [(header::SET_COOKIE, csrf_cookie.to_string())],
        Redirect::to(auth_url.as_str()),
    )
}
```

### Callback Handler

```rust
pub async fn auth_callback(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    // Verify CSRF
    let csrf_cookie = cookies.get("oauth_csrf")
        .ok_or(AppError::BadRequest("Missing CSRF token"))?;
    if csrf_cookie.value() != params.state {
        return Err(AppError::BadRequest("CSRF mismatch"));
    }

    // Determine provider from state or separate param
    let client = google_client(); // or microsoft based on callback

    // Exchange code for token
    let token = client
        .exchange_code(AuthorizationCode::new(params.code))
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| AppError::Internal(format!("Token exchange failed: {}", e)))?;

    // Decode ID token to get user info
    let id_token = token.extra_fields().id_token()
        .ok_or(AppError::Internal("No ID token"))?;
    let claims = decode_id_token(id_token)?;

    // Check allowed domains
    if let Some(allowed) = &state.config.allowed_email_domains {
        let domain = claims.email.split('@').last().unwrap_or("");
        if !allowed.contains(&domain.to_string()) {
            return Err(AppError::Forbidden("Email domain not allowed"));
        }
    }

    // Upsert user
    let user = upsert_user(&state.pool, &claims).await?;

    // Create session
    let session_token = generate_session_token();
    let token_hash = sha256(&session_token);
    create_session(&state.pool, user.id, &token_hash).await?;

    // Set session cookie
    let mut cookie = Cookie::new(SESSION_COOKIE, session_token);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(time::Duration::days(7));

    Ok((
        [(header::SET_COOKIE, cookie.to_string())],
        Redirect::to("/"),
    ))
}
```

## UI Changes

### Login Page

```tsx
// toygres-ui/src/components/Login.tsx

export function Login() {
  return (
    <div className="login-container">
      <h1>Toygres</h1>
      <p>PostgreSQL Control Plane</p>

      <div className="oauth-buttons">
        <a href="/auth/login/google" className="oauth-btn google">
          <GoogleIcon />
          Sign in with Google
        </a>

        <a href="/auth/login/microsoft" className="oauth-btn microsoft">
          <MicrosoftIcon />
          Sign in with Microsoft
        </a>
      </div>
    </div>
  );
}
```

### User Menu

```tsx
// Show logged-in user in header
const { data: user } = useQuery(['me'], () => api.get('/api/me'));

<div className="user-menu">
  <img src={user.avatar_url} alt={user.name} />
  <span>{user.name}</span>
  <button onClick={() => window.location.href = '/auth/logout'}>
    Logout
  </button>
</div>
```

## Session Cleanup

Add a system orchestration to prune expired sessions:

```rust
// orchestrations/session_pruner.rs

pub async fn session_pruner(ctx: OrchestrationContext) -> Result<(), String> {
    // Delete expired sessions
    ctx.schedule_activity_typed::<_, ()>(
        "prune-expired-sessions",
        &PruneSessionsInput { older_than_hours: 168 },
    ).await?;

    // Run daily
    ctx.schedule_timer(Duration::from_secs(86400)).into_timer().await;
    ctx.continue_as_new("{}").await?;

    Ok(())
}
```

## File Changes Summary

```
toygres-server/src/
├── auth.rs              # Rewrite: OAuth middleware
├── oauth.rs             # New: OAuth client setup
├── routes/
│   └── auth.rs          # New: /auth/* handlers
└── main.rs              # Add auth routes

migrations/cms/
└── 0007_add_oauth_users.sql

toygres-ui/src/
├── components/
│   └── Login.tsx        # New: OAuth login page
└── App.tsx              # Update: user context

.env.example             # Add OAuth vars
```

## Security Considerations

1. **CSRF protection** - State parameter verified on callback
2. **Secure cookies** - HttpOnly, Secure, SameSite=Lax
3. **Token hashing** - Store SHA256 of session token, not plaintext
4. **Domain allowlist** - Optional restriction to specific email domains
5. **Session expiry** - 7-day default, configurable
6. **No refresh tokens stored** - Stateless after initial auth

## Migration Path

1. Deploy with OAuth enabled + legacy auth
2. Create admin users via OAuth
3. Disable legacy `TOYGRES_ADMIN_USERNAME/PASSWORD`
4. Remove legacy auth code
