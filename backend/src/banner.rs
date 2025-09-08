// src/banner.rs
use actix_web::{get, post, put, delete, web, HttpResponse, Responder};
use serde::Deserialize;
use crate::db::Database;
use crate::middleware::AdminAuth;

// --- 请求体定义 (从 admin.rs 移过来) ---
#[derive(Deserialize)]
pub struct CreateBannerRequest {
    pub image_url: String,
    pub link_url: String,
}

#[derive(Deserialize)]
pub struct UpdateBannerRequest {
    pub image_url: String,
    pub link_url: String,
}


// --- 公共 API ---

#[get("/banners")]
pub async fn get_banners(db: web::Data<Database>) -> impl Responder {
    println!("API Call: /api/banners received.");
    match db.get_all_banners() {
        Ok(banners) => {
            println!("API Success: /api/banners - Fetched {} banners.", banners.len());
            HttpResponse::Ok().json(banners)
        }
        Err(e) => {
            eprintln!("API Error: /api/banners - Failed to fetch banners: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch banners"}))
        }
    }
}

// --- 管理员 API (现在也放在这个文件里) ---

#[post("/banners", wrap = "AdminAuth")]
pub async fn create_banner(
    db: web::Data<Database>,
    req: web::Json<CreateBannerRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/banners - Received request to create a banner.");
    match db.create_banner(&req.image_url, &req.link_url) {
        Ok(banner_id) => {
            println!("API Success: /api/admin/banners - Banner created with ID: {}", banner_id);
            HttpResponse::Created().json(serde_json::json!({
                "message": "Banner created successfully",
                "id": banner_id
            }))
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/banners - Failed to create banner: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create banner"}))
        }
    }
}

#[get("/banners/all", wrap = "AdminAuth")]
pub async fn get_all_banners_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/banners/all - Received request to get all banners.");
    match db.get_all_banners() {
        Ok(banners) => {
            println!("API Success: /api/admin/banners/all - Fetched {} banners.", banners.len());
            HttpResponse::Ok().json(banners)
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/banners/all - Failed to fetch banners: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch banners"}))
        }
    }
}

#[put("/banners/{id}", wrap = "AdminAuth")]
pub async fn update_banner(
    db: web::Data<Database>,
    path: web::Path<i64>,
    req: web::Json<UpdateBannerRequest>,
) -> impl Responder {
    let banner_id = path.into_inner();
    println!("API Info: /api/admin/banners/{} - Received request to update banner.", banner_id);
    match db.update_banner(banner_id, &req.image_url, &req.link_url) {
        Ok(_) => {
            println!("API Success: /api/admin/banners/{} - Banner updated successfully.", banner_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "Banner updated successfully"}))
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/banners/{} - Failed to update banner: {:?}", banner_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to update banner"}))
        }
    }
}

#[delete("/banners/{id}", wrap = "AdminAuth")]
pub async fn delete_banner(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let banner_id = path.into_inner();
    println!("API Info: /api/admin/banners/{} - Received request to delete banner.", banner_id);
    match db.delete_banner(banner_id) {
        Ok(_) => {
            println!("API Success: /api/admin/banners/{} - Banner deleted successfully.", banner_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "Banner deleted successfully"}))
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/banners/{} - Failed to delete banner: {:?}", banner_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to delete banner"}))
        }
    }
}