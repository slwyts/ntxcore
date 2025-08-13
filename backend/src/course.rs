// src/course.rs
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest, put, delete};
use serde::{Deserialize};
use crate::db::Database;
use crate::middleware::AdminAuth; // 管理员权限验证
use crate::user::get_user_id_from_token; // 获取用户ID
use crate::JwtConfig;
use crate::db::{CourseDetails, PermissionGroupInfo};
use std::collections::{HashMap, HashSet};
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

// 用于更新的请求体
#[derive(Deserialize)]
pub struct UpdatePermissionGroupRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateCoursePackageRequest {
    pub group_id: i64,
    pub duration_days: i64,
    pub price: f64,
    pub currency: String,
}

#[derive(Deserialize)]
pub struct UpdateCourseRequest {
    pub course_type: String,
    pub name: String,
    pub description: String,
    pub content: String,
}

// 用于手动授权的请求体
#[derive(Deserialize)]
pub struct GrantPermissionRequest {
    pub group_id: i64,
    pub duration_days: i64,
}

#[derive(Deserialize)]
pub struct RevokePermissionRequest {
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

/// 获取所有课程列表，并标记用户访问状态
#[get("/all")]
pub async fn get_all_courses_for_user(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    // 1. 获取用户ID，如果token无效则用户ID为-1 (匿名用户)
    let user_id = get_user_id_from_token(&req, &jwt_config).unwrap_or(-1);
    
    // 2. 获取该用户拥有的所有有效权限ID
    let user_permission_ids = if user_id != -1 {
        db.get_user_active_permission_ids(user_id).unwrap_or_default()
    } else {
        // 匿名用户只拥有默认权限
        let mut default_perms = HashSet::new();
        default_perms.insert(1);
        default_perms
    };

    // 3. 获取所有课程及其所需的权限组信息
    let all_courses_with_groups = match db.get_all_courses_with_their_groups() {
        Ok(data) => data,
        Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    };

    // 4. 将数据重组成以 course_id为key的 HashMap，方便处理
    let mut courses_map: HashMap<i64, CourseDetails> = HashMap::new();

    for item in all_courses_with_groups {
        let course = courses_map.entry(item.course_id).or_insert_with(|| CourseDetails {
            id: item.course_id,
            course_type: item.course_type.clone(),
            name: item.course_name.clone(),
            description: item.course_description.clone(),
            content: item.course_content.clone(), // 先临时保存
            is_unlocked: false, // 默认为未解锁
            required_groups: Vec::new(),
        });
        // 添加解锁当前课程所需的权限组信息
        course.required_groups.push(PermissionGroupInfo { id: item.group_id, name: item.group_name });
    }

    // 5. 最终处理，决定每个课程是否解锁并处理内容
    let result: Vec<CourseDetails> = courses_map.into_values().map(|mut course| {
        // 检查用户拥有的权限是否包含解锁该课程所需的任一权限
        let is_unlocked = course.required_groups.iter().any(|group| user_permission_ids.contains(&group.id));
        course.is_unlocked = is_unlocked;

        if !is_unlocked {
            course.content = "".to_string(); // 如果未解锁，清空内容
        }
        course
    }).collect();

    HttpResponse::Ok().json(result)
}






#[get("/permission_groups/all", wrap="AdminAuth")]
pub async fn get_all_permission_groups_admin(db: web::Data<Database>) -> impl Responder {
    match db.get_all_permission_groups() {
        Ok(groups) => HttpResponse::Ok().json(groups),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[put("/permission_groups/{id}", wrap="AdminAuth")]
pub async fn update_permission_group(db: web::Data<Database>, path: web::Path<i64>, req: web::Json<UpdatePermissionGroupRequest>) -> impl Responder {
    let group_id = path.into_inner();
    match db.update_permission_group(group_id, &req.name) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "权限组更新成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[delete("/permission_groups/{id}", wrap="AdminAuth")]
pub async fn delete_permission_group(db: web::Data<Database>, path: web::Path<i64>) -> impl Responder {
    let group_id = path.into_inner();
    match db.delete_permission_group(group_id) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "权限组删除成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// --- 课程管理 ---
#[get("/courses/all", wrap="AdminAuth")]
pub async fn get_all_courses_admin(db: web::Data<Database>) -> impl Responder {
    match db.get_all_courses() {
        Ok(courses) => HttpResponse::Ok().json(courses),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[put("/courses/{id}", wrap="AdminAuth")]
pub async fn update_course(db: web::Data<Database>, path: web::Path<i64>, req: web::Json<UpdateCourseRequest>) -> impl Responder {
    let course_id = path.into_inner();
    match db.update_course(course_id, &req.course_type, &req.name, &req.description, &req.content) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "课程更新成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[delete("/courses/{id}", wrap="AdminAuth")]
pub async fn delete_course(db: web::Data<Database>, path: web::Path<i64>) -> impl Responder {
    let course_id = path.into_inner();
    match db.delete_course(course_id) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "课程删除成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

// --- 课程套餐管理 ---
#[get("/course_packages/all", wrap="AdminAuth")]
pub async fn get_all_course_packages_admin(db: web::Data<Database>) -> impl Responder {
    match db.get_all_course_packages() {
        Ok(packages) => HttpResponse::Ok().json(packages),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[put("/course_packages/{id}", wrap="AdminAuth")]
pub async fn update_course_package(db: web::Data<Database>, path: web::Path<i64>, req: web::Json<UpdateCoursePackageRequest>) -> impl Responder {
    let package_id = path.into_inner();
    match db.update_course_package(package_id, req.group_id, req.duration_days, req.price, &req.currency) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "课程套餐更新成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

#[delete("/course_packages/{id}", wrap="AdminAuth")]
pub async fn delete_course_package(db: web::Data<Database>, path: web::Path<i64>) -> impl Responder {
    let package_id = path.into_inner();
    match db.delete_course_package(package_id) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "课程套餐删除成功"})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()})),
    }
}

