use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpResponse
};
use actix_web::body::BoxBody;
use futures_util::future::{self, LocalBoxFuture};
use std::rc::Rc;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use crate::auth::Claims;
use crate::JwtConfig;
use crate::db::Database;

// 新增 AdminKeyConfig 结构体用于存放 KEY
#[derive(Clone)]
pub struct AdminKeyConfig {
    pub key: String,
}

pub struct AdminAuth;

impl<S> Transform<S, ServiceRequest> for AdminAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = AdminAuthMiddleware<S>;
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ready(Ok(AdminAuthMiddleware { service: Rc::new(service) }))
    }
}

pub struct AdminAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S> Service<ServiceRequest> for AdminAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let jwt_config = req.app_data::<web::Data<JwtConfig>>().cloned();
        let db = req.app_data::<web::Data<Database>>().cloned();
        // 获取 AdminKeyConfig
        let admin_key_config = req.app_data::<web::Data<AdminKeyConfig>>().cloned();
        let auth_header = req.headers().get("Authorization").cloned();
        // 尝试获取 X-API-KEY 头部
        let api_key_header = req.headers().get("X-API-KEY").cloned();

        Box::pin(async move {
            let (jwt_config, db, admin_key_config) = match (jwt_config, db, admin_key_config) {
                (Some(jc), Some(d), Some(akc)) => (jc, d, akc),
                _ => {
                    eprintln!("[AdminAuth] Error: JWT config, DB, or AdminKey config not found in app data.");
                    let resp = HttpResponse::InternalServerError().finish();
                    return Ok(req.into_response(resp).map_into_boxed_body());
                }
            };

            // 优先检查 X-API-KEY
            if let Some(header_value) = api_key_header {
                if let Ok(key_str) = header_value.to_str() {
                    // 如果传入的 KEY 和系统配置的 KEY 匹配，则直接放行
                    if key_str == admin_key_config.key {
                        println!("[AdminAuth] Info: Access granted via X-API-KEY.");
                        let res = service.call(req).await?;
                        return Ok(res.map_into_boxed_body());
                    } else {
                        eprintln!("[AdminAuth] Error: Invalid X-API-KEY provided.");
                        let resp = HttpResponse::Forbidden().json("Invalid API Key");
                        return Ok(req.into_response(resp).map_into_boxed_body());
                    }
                }
            }

            // 如果没有 X-API-KEY 或者 X-API-KEY 不匹配，则继续 JWT 验证流程
            let token_str = match auth_header {
                Some(header_value) => {
                    match header_value.to_str() {
                        Ok(s) if s.starts_with("Bearer ") => s.trim_start_matches("Bearer ").to_string(),
                        _ => {
                            eprintln!("[AdminAuth] Error: Invalid Authorization header format.");
                            let resp = HttpResponse::Forbidden().json("Invalid token format");
                            return Ok(req.into_response(resp).map_into_boxed_body());
                        }
                    }
                },
                None => {
                    // 如果两者都缺失，则拒绝访问
                    eprintln!("[AdminAuth] Error: Authorization header or X-API-KEY missing.");
                    let resp = HttpResponse::Forbidden().json("Authorization token or API Key required");
                    return Ok(req.into_response(resp).map_into_boxed_body());
                }
            };

            // JWT 验证
            let decoding_key = DecodingKey::from_secret(jwt_config.secret.as_bytes());
            let validation = Validation::new(Algorithm::HS256);
            let token_data = match decode::<Claims>(&token_str, &decoding_key, &validation) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("[AdminAuth] Error: Token decoding failed: {:?}", e);
                    let resp = HttpResponse::Forbidden().json("Invalid or expired token");
                    return Ok(req.into_response(resp).map_into_boxed_body());
                },
            };

            let user_id = token_data.claims.sub;

            match db.is_user_admin(user_id) {
                Ok(true) => {
                    let res = service.call(req).await?;
                    Ok(res.map_into_boxed_body())
                },
                Ok(false) => {
                    eprintln!("[AdminAuth] Warning: Non-admin user {} attempted to access admin route.", user_id);
                    let resp = HttpResponse::Forbidden().json("Access denied: Administrator privileges required.");
                    Ok(req.into_response(resp).map_into_boxed_body())
                },
                Err(e) => {
                    eprintln!("[AdminAuth] Error: Failed to check admin status for user {}: {:?}", user_id, e);
                    let resp = HttpResponse::InternalServerError().json("Failed to verify user privileges");
                    Ok(req.into_response(resp).map_into_boxed_body())
                }
            }
        })
    }
}