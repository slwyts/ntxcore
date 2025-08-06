// src/main.rs
mod auth;
mod mining;
mod db;
mod utils;
mod user;
mod settlement;
mod admin;
mod middleware; // 引入中间件模块
mod tasks;

use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use std::env;
use db::Database;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use crate::middleware::{AdminAuth, AdminKeyConfig}; // 导入 AdminAuth 中间件 和 AdminKeyConfig

// JwtConfig
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
}

// MailConfig
#[derive(Clone)]
pub struct MailConfig {
    pub user: String,
    pub pass: String,
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // 初始化日志
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // 获取环境变量
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let db_file = env::var("DB_FILE").expect("DB_FILE 环境变量未设置");
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET 环境变量未设置");
    let mail_user = env::var("MAIL_USER").expect("MAIL_USER 环境变量未设置");
    let mail_pass = env::var("MAIL_PASS").expect("MAIL_PASS 环境变量未设置");
    let key = env::var ("KEY").expect("system KEY is not set");
    // 创建数据库实例
    let db = match Database::new(&db_file) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("数据库初始化失败: {}", e);
            return Ok(());
        }
    };
    let db_data = web::Data::new(db);

    // 创建配置实例
    let mail_config = web::Data::new(MailConfig {
        user: mail_user,
        pass: mail_pass,
    });
    let jwt_config = web::Data::new(JwtConfig {
        secret: jwt_secret,
    });
    // 新增 AdminKeyConfig
    let admin_key_config = web::Data::new(AdminKeyConfig {
        key: key,
    });

    tasks::start_scheduled_tasks(db_data.clone()).await;
    // 启动任务调度
    println!("任务调度已启动");
    // 启动 HTTP 服务器
    let bind_address = format!("0.0.0.0:{}", port);
    println!("服务器启动在: http://{}", bind_address);

    HttpServer::new(move || {
        // 配置 CORS
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(db_data.clone())
            .app_data(mail_config.clone())
            .app_data(jwt_config.clone())
            .app_data(admin_key_config.clone()) // 将 AdminKeyConfig 传递给 App
            .service(
                web::scope("/api/auth")
                    .service(auth::register)
                    .service(auth::login)
                    .service(auth::send_verification_code)
                    .service(auth::forgot_password)
                    .service(auth::reset_password)
                    .service(auth::update_user_password_with_old)
            )
            .service(
                web::scope("/api/mining")
                    .service(mining::get_platform_data)
                    .service(mining::get_daily_platform_data)
                    .service(mining::get_exchanges)
                    .service(mining::get_user_data)
                    .service(mining::get_daily_user_data)
                    .service(mining::get_user_exchanges)
                    .service(mining::bind_exchange)
                    .service(mining::get_mining_leaderboard)
            )
            .service(
                web::scope("/api/user")
                    .service(user::get_user_info)
                    .service(user::want_withdraw_usdt)
                    .service(user::want_withdraw_ntx)
                    .service(user::get_my_teams)
                    .service(user::get_commission_records)
                    .service(user::get_user_withdrawal_records)
                    .service(user::bind_bsc_address)
                    .service(user::get_current_dao_auction)
                    .service(user::get_articles) // 获取文章列表
                    .service(user::get_article_detail) // 获取文章详情
                    .service(user::update_user_nickname)
            )
            .service(
                web::scope("/api/admin")
                    .wrap(AdminAuth)
                    .service(admin::get_dashboard_data) // 仪表盘API
                    .service(admin::get_all_users)
                    .service(admin::add_user_by_admin) // 管理员添加用户
                    .service(admin::get_user_full_info)
                    .service(admin::delete_user_by_admin)
                    .service(admin::get_user_bound_exchanges)
                    .service(admin::get_all_exchanges_admin) // 获取所有交易所
                    .service(admin::add_daily_trade_data)
                    .service(admin::get_daily_trades_admin) // 获取指定日期的所有用户交易记录
                    .service(admin::create_exchange)
                    .service(admin::update_exchange)
                    .service(admin::delete_exchange)
                    .service(admin::update_exchange_mining_efficiency)
                    .service(admin::toggle_user_status)
                    .service(admin::get_all_withdrawal_orders)
                    .service(admin::update_withdrawal_order_status)
                    .service(admin::update_user_total_data)
                    .service(admin::update_daily_user_data)
                    .service(admin::get_user_daily_data_history) // 获取用户指定日期范围的每日数据
                    .service(admin::update_platform_total_data)
                    .service(admin::update_daily_platform_data)
                    .service(admin::get_platform_data_history) // 获取历史平台数据
                    .service(admin::update_user_profile)
                    .service(admin::start_dao_auction)
                    .service(admin::end_dao_auction)
                    .service(admin::get_all_dao_auctions_admin) // 获取所有DAO拍卖历史
                    .service(admin::get_all_user_bsc_addresses)
                    .service(admin::publish_article) // 发布文章
                    .service(admin::modify_article) // 修改文章
                    .service(admin::delete_article) // 
                    .service(admin::get_all_articles_admin) // 
                    .service(admin::get_article_detail_admin) // 管理员获取文章详情
                    .service(admin::get_all_referral_relationships_admin) // 获取所有推荐关系
                    .service(admin::get_all_commissions_admin) // 获取所有佣金记录
                    .service(admin::get_commissions_summary_by_inviter_admin) // 按邀请人汇总佣金数据
                    .service(admin::get_financial_summary_admin) // 获取财务汇总信息
                    .service(admin::update_ntx_control_percentage)// 新增：更新NTX分配控制的目标百分比
            )
            .service(
                web::scope("/api/system")
                    .wrap(AdminAuth) 
                    .service(settlement::trigger_daily_settlement) 
                    .service(admin::get_all_user_gntx_info)
                    .service(admin::update_user_gntx_balance_admin)
                    .service(admin::get_exchange_bound_users_admin)
                    .service(settlement::force_ntx_control)
            )
    })
    .bind(&bind_address)?
    .run()
    .await
}
