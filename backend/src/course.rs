// src/course.rs
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use serde::Deserialize;
use crate::db::Database;
use crate::middleware::AdminAuth; // 管理员权限验证
use crate::user::get_user_id_from_token; // 获取用户ID
use crate::JwtConfig;

// --- 请求体定义 ---

#[derive(Deserialize)]
pub struct CreatePermissionGroupRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateCoursePackageRequest {
    pub group_id: i64,
    pub duration_days: i64,
    pub price: f64,
    pub currency: String,
}

#[derive(Deserialize)]
pub struct CreateCourseRequest {
    pub course_type: String,
    pub name: String,
    pub description: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct AssignCourseToGroupRequest {
    pub group_id: i64,
}

// --- Admin 路由处理函数 ---

// 创建权限组
#[post("/permission_groups", wrap="AdminAuth")]
pub async fn create_permission_group(
    db: web::Data<Database>,
    req: web::Json<CreatePermissionGroupRequest>,
) -> impl Responder {
    match db.create_permission_group(&req.name) {
        Ok(group_id) => HttpResponse::Ok().json(serde_json::json!({
            "message": "权限组创建成功",
            "id": group_id
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// 创建课程套餐
#[post("/course_packages", wrap="AdminAuth")]
pub async fn create_course_package(
    db: web::Data<Database>,
    req: web::Json<CreateCoursePackageRequest>,
) -> impl Responder {
    match db.create_course_package(req.group_id, req.duration_days, req.price, &req.currency) {
        Ok(package_id) => HttpResponse::Ok().json(serde_json::json!({
            "message": "课程套餐创建成功",
            "id": package_id
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// 创建课程
#[post("/courses", wrap="AdminAuth")]
pub async fn create_course(
    db: web::Data<Database>,
    req: web::Json<CreateCourseRequest>,
) -> impl Responder {
    match db.create_course(&req.course_type, &req.name, &req.description, &req.content) {
        Ok(course_id) => HttpResponse::Ok().json(serde_json::json!({
            "message": "课程创建成功",
            "id": course_id
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// 为课程分配权限组
#[post("/courses/{course_id}/assign_group", wrap="AdminAuth")]
pub async fn assign_course_to_group(
    db: web::Data<Database>,
    path: web::Path<i64>,
    req: web::Json<AssignCourseToGroupRequest>,
) -> impl Responder {
    let course_id = path.into_inner();
    match db.assign_course_to_group(course_id, req.group_id) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "课程成功分配给权限组"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// --- User 路由处理函数 ---

// 获取所有权限组及其套餐
#[get("/permission_groups")]
pub async fn get_all_groups_and_packages(db: web::Data<Database>) -> impl Responder {
    match db.get_all_permission_groups() {
        Ok(groups) => {
            let mut result = vec![];
            for group in groups {
                let packages = db.get_packages_for_group(group.id).unwrap_or_default();
                result.push(serde_json::json!({
                    "group": group,
                    "packages": packages,
                }));
            }
            HttpResponse::Ok().json(result)
        },
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// 获取用户有权访问的课程列表
#[get("/my_courses")]
pub async fn get_my_courses(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    
    match db.get_accessible_courses_for_user(user_id) {
        Ok(courses) => HttpResponse::Ok().json(courses),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()}))
    }
}