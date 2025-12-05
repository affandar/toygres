use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use tower_cookies::{Cookie, Cookies};

const SESSION_COOKIE: &str = "toygres_session";
const SESSION_TOKEN: &str = "authenticated_toygres_admin_session";

/// Get admin username from environment (TOYGRES_ADMIN_USERNAME)
/// Panics if not set - credentials must be configured in .env
fn get_admin_username() -> String {
    std::env::var("TOYGRES_ADMIN_USERNAME")
        .expect("TOYGRES_ADMIN_USERNAME must be set in .env file")
}

/// Get admin password from environment (TOYGRES_ADMIN_PASSWORD)
/// Panics if not set - credentials must be configured in .env
fn get_admin_password() -> String {
    std::env::var("TOYGRES_ADMIN_PASSWORD")
        .expect("TOYGRES_ADMIN_PASSWORD must be set in .env file")
}

/// Login page HTML
const LOGIN_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Toygres - Login</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', monospace;
            background: linear-gradient(135deg, #0f0f23 0%, #1a1a3e 50%, #0f0f23 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #e0e0e0;
        }
        
        .login-container {
            background: rgba(20, 20, 40, 0.9);
            border: 1px solid #3d3d6b;
            border-radius: 12px;
            padding: 48px;
            width: 100%;
            max-width: 420px;
            box-shadow: 
                0 20px 60px rgba(0, 0, 0, 0.5),
                0 0 40px rgba(100, 100, 255, 0.1);
        }
        
        .logo {
            text-align: center;
            margin-bottom: 32px;
        }
        
        .logo h1 {
            font-size: 2.5rem;
            font-weight: 700;
            background: linear-gradient(135deg, #6b8cff 0%, #a855f7 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
            letter-spacing: -1px;
        }
        
        .logo span {
            display: block;
            font-size: 0.75rem;
            color: #8888aa;
            margin-top: 8px;
            letter-spacing: 2px;
            text-transform: uppercase;
        }
        
        .form-group {
            margin-bottom: 24px;
        }
        
        label {
            display: block;
            font-size: 0.75rem;
            color: #8888aa;
            margin-bottom: 8px;
            text-transform: uppercase;
            letter-spacing: 1px;
        }
        
        input {
            width: 100%;
            padding: 14px 16px;
            background: rgba(30, 30, 60, 0.8);
            border: 1px solid #3d3d6b;
            border-radius: 8px;
            color: #e0e0e0;
            font-size: 1rem;
            font-family: inherit;
            transition: all 0.2s ease;
        }
        
        input:focus {
            outline: none;
            border-color: #6b8cff;
            box-shadow: 0 0 0 3px rgba(107, 140, 255, 0.2);
        }
        
        input::placeholder {
            color: #555577;
        }
        
        button {
            width: 100%;
            padding: 16px;
            background: linear-gradient(135deg, #6b8cff 0%, #a855f7 100%);
            border: none;
            border-radius: 8px;
            color: white;
            font-size: 1rem;
            font-weight: 600;
            font-family: inherit;
            cursor: pointer;
            transition: all 0.2s ease;
            text-transform: uppercase;
            letter-spacing: 1px;
        }
        
        button:hover {
            transform: translateY(-2px);
            box-shadow: 0 8px 20px rgba(107, 140, 255, 0.3);
        }
        
        button:active {
            transform: translateY(0);
        }
        
        .error-message {
            background: rgba(239, 68, 68, 0.15);
            border: 1px solid rgba(239, 68, 68, 0.3);
            color: #f87171;
            padding: 12px 16px;
            border-radius: 8px;
            margin-bottom: 24px;
            font-size: 0.875rem;
            text-align: center;
            display: none;
        }
        
        .error-message.show {
            display: block;
        }
        
        .footer {
            text-align: center;
            margin-top: 32px;
            font-size: 0.75rem;
            color: #555577;
        }
    </style>
</head>
<body>
    <div class="login-container">
        <div class="logo">
            <h1>🐘 Toygres</h1>
            <span>PostgreSQL Control Plane</span>
        </div>
        
        <div id="error" class="error-message"></div>
        
        <form id="loginForm" method="POST" action="/login">
            <div class="form-group">
                <label for="username">Username</label>
                <input 
                    type="text" 
                    id="username" 
                    name="username" 
                    placeholder="Enter your username"
                    autocomplete="username"
                    required
                >
            </div>
            
            <div class="form-group">
                <label for="password">Password</label>
                <input 
                    type="password" 
                    id="password" 
                    name="password" 
                    placeholder="Enter your password"
                    autocomplete="current-password"
                    required
                >
            </div>
            
            <button type="submit">Sign In</button>
        </form>
        
        <div class="footer">
            Secure access to your PostgreSQL instances
        </div>
    </div>
    
    <script>
        const form = document.getElementById('loginForm');
        const errorDiv = document.getElementById('error');
        
        // Check for error in URL params
        const urlParams = new URLSearchParams(window.location.search);
        if (urlParams.get('error') === 'invalid') {
            errorDiv.textContent = 'Invalid username or password';
            errorDiv.classList.add('show');
        }
    </script>
</body>
</html>"#;

/// Serve the login page
pub async fn login_page() -> Html<&'static str> {
    Html(LOGIN_PAGE)
}

/// Handle login form submission
#[derive(serde::Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

pub async fn login_handler(
    cookies: Cookies,
    axum::Form(form): axum::Form<LoginForm>,
) -> impl IntoResponse {
    if form.username == get_admin_username() && form.password == get_admin_password() {
        // Set session cookie
        let mut cookie = Cookie::new(SESSION_COOKIE, SESSION_TOKEN);
        cookie.set_path("/");
        cookie.set_http_only(true);
        cookies.add(cookie);
        
        Redirect::to("/").into_response()
    } else {
        Redirect::to("/login?error=invalid").into_response()
    }
}

/// Handle logout
pub async fn logout_handler(cookies: Cookies) -> impl IntoResponse {
    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookies.remove(cookie);
    
    Redirect::to("/login")
}

/// Check if request is authenticated via session cookie
fn is_authenticated(cookies: &Cookies) -> bool {
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        if cookie.value() == SESSION_TOKEN {
            return true;
        }
    }
    false
}

/// Authentication middleware
pub async fn auth_middleware(
    cookies: Cookies,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    
    // Public routes that don't require auth
    if path == "/login" || path == "/health" || path.starts_with("/static/") {
        return next.run(req).await;
    }
    
    // Check authentication via session cookie
    if is_authenticated(&cookies) {
        return next.run(req).await;
    }
    
    // For API requests, return 401
    if path.starts_with("/api/") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Authentication required. Please login at /login"})),
        ).into_response();
    }
    
    // For browser requests, redirect to login
    Redirect::to("/login").into_response()
}

