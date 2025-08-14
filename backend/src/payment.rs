// src/payment.rs
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use serde::{Deserialize};
use std::env;
use crate::db::Database;
use crate::middleware::AdminAuth;
use crate::user::get_user_id_from_token;
use crate::JwtConfig;
use rand::Rng;

// --- 请求体定义 ---

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub package_id: i64,
}

#[derive(Deserialize)]
pub struct OrderQuery {
    pub status: Option<String>,
}

// --- 路由处理函数 ---

// 用户创建订单
#[post("/orders")]
pub async fn create_order(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    order_req: web::Json<CreateOrderRequest>,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    
    // 1. 获取套餐信息以确定价格和货币
    let package = match db.get_package_by_id(order_req.package_id) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::NotFound().json(serde_json::json!({"error": "套餐不存在"})),
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    };
    
    // 2. 生成唯一的支付金额
    // 生成一个 0.00001 到 0.00999 之间的随机数
    let random_micro_amount: f64 = rand::thread_rng().gen_range(1..1000) as f64 / 100_000.0;
    // 【修复】修正计算逻辑，避免浮点数精度问题
    // 先将价格和偏移量都放大为整数，相加后再缩小
    let price_in_base = (package.price * 100_000.0).round();
    let offset_in_base = (random_micro_amount * 100_000.0).round();
    let payment_amount = (price_in_base + offset_in_base) / 100_000.0;

    // 3. 创建订单，并存入新的支付金额
    match db.create_order(user_id, order_req.package_id, package.price, payment_amount, &package.currency) {
        Ok(order_id) => {
            // 从环境变量获取收款地址
            let receiving_address = env::var("PAYMENT_RECEIVING_ADDRESS")
                .unwrap_or_else(|_| "YOUR_DEFAULT_WALLET_ADDRESS_NOT_SET".to_string());

            HttpResponse::Ok().json(serde_json::json!({
                "message": "订单创建成功，请支付",
                "orderId": order_id,
                "amount": package.price, // 原始套餐价格
                "paymentAmount": payment_amount, // 要求用户实际支付的唯一金额
                "currency": package.currency,
                "paymentAddress": receiving_address // 收款地址
            }))
        },
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}


// 用户获取自己的订单列表
#[get("/orders")]
pub async fn get_my_orders(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match db.get_user_orders(user_id) {
        Ok(orders) => HttpResponse::Ok().json(orders),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// 管理员确认订单支付
#[post("/orders/{order_id}/confirm", wrap="AdminAuth")]
pub async fn confirm_order_payment(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let order_id = path.into_inner();

    // 1. 获取订单和套餐信息
    let order = match db.get_order_by_id(order_id) {
        Ok(Some(o)) => o,
        Ok(None) => return HttpResponse::NotFound().json(serde_json::json!({"error": "订单不存在"})),
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    };
    
    let package = match db.get_package_by_id(order.package_id) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::NotFound().json(serde_json::json!({"error": "订单关联的套餐不存在"})),
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    };
    
    // 2. 更新订单状态为 "confirmed"
    if let Err(e) = db.update_order_status(order_id, "confirmed") {
        return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()}));
    }

    // 3. 为用户授予权限
    if let Err(e) = db.grant_permission_to_user(order.user_id, package.group_id, package.duration_days) {
        // 即便授权失败，订单状态已经更新，这里只记录错误
        eprintln!("Error granting permission for order {}: {}", order_id, e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "订单状态已更新，但权限授予失败",
            "details": e.to_string()
        }));
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "message": "订单已手动确认，并成功为用户授予权限"
    }))
}

#[get("/orders/all", wrap="AdminAuth")]
pub async fn get_all_orders_admin(
    db: web::Data<Database>,
    query: web::Query<OrderQuery>
) -> impl Responder {
    match db.get_all_orders(query.status.as_deref()) {
        Ok(orders) => HttpResponse::Ok().json(orders),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}