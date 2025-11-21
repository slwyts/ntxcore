# 邮件发送系统 API 文档

## 📋 目录

- [概述](#概述)
- [认证](#认证)
- [数据结构](#数据结构)
- [邮件模板管理](#邮件模板管理)
- [邮件任务管理](#邮件任务管理)
- [使用流程](#使用流程)
- [错误处理](#错误处理)

---

## 概述

邮件发送系统提供了完整的邮件模板管理和批量发送功能，支持HTML模板和变量占位符替换。

### 主要功能

- ✅ HTML 邮件模板的 CRUD 操作
- ✅ 支持 `{{变量名}}` 占位符自动替换
- ✅ 批量发送邮件（异步执行）
- ✅ 详细的发送日志和统计
- ✅ 自动发送间隔控制（防止被封号）

### 基础 URL

```
所有邮件 API 的基础路径：/api/admin/email
```

---

## 认证

所有邮件 API 都需要管理员权限，必须在请求头中携带有效的管理员 JWT Token。

### 请求头

```http
Authorization: Bearer <your-admin-jwt-token>
```

---

## 数据结构

### EmailTemplate (邮件模板)

```json
{
  "id": 1,
  "name": "模板名称",
  "subject": "邮件主题 {{变量}}",
  "htmlContent": "<h1>HTML内容 {{变量}}</h1>",
  "createdAt": "2025-01-01T00:00:00Z",
  "updatedAt": "2025-01-01T00:00:00Z"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| id | integer | 模板ID |
| name | string | 模板名称（唯一） |
| subject | string | 邮件主题，支持占位符 |
| htmlContent | string | HTML邮件内容，支持占位符 |
| createdAt | string | 创建时间 (RFC3339) |
| updatedAt | string | 更新时间 (RFC3339) |

### EmailTask (邮件任务)

```json
{
  "id": 1,
  "templateId": 1,
  "variables": "{\"username\":\"张三\",\"code\":\"123456\"}",
  "recipients": "[\"user1@example.com\",\"user2@example.com\"]",
  "status": "completed",
  "createdAt": "2025-01-01T00:00:00Z",
  "startedAt": "2025-01-01T00:00:05Z",
  "completedAt": "2025-01-01T00:01:00Z",
  "totalCount": 100,
  "successCount": 98,
  "failedCount": 2
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| id | integer | 任务ID |
| templateId | integer | 使用的模板ID |
| variables | string | JSON格式的变量映射 |
| recipients | string | JSON格式的收件人列表 |
| status | string | 任务状态：pending/processing/completed/failed |
| createdAt | string | 创建时间 |
| startedAt | string? | 开始执行时间 |
| completedAt | string? | 完成时间 |
| totalCount | integer | 总收件人数 |
| successCount | integer | 成功发送数 |
| failedCount | integer | 失败发送数 |

### EmailLog (邮件日志)

```json
{
  "id": 1,
  "taskId": 1,
  "recipient": "user@example.com",
  "status": "success",
  "errorMessage": null,
  "sentAt": "2025-01-01T00:00:10Z"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| id | integer | 日志ID |
| taskId | integer | 所属任务ID |
| recipient | string | 收件人邮箱 |
| status | string | 发送状态：success/failed |
| errorMessage | string? | 错误信息（失败时） |
| sentAt | string | 发送时间 |

---

## 邮件模板管理

### 1. 创建邮件模板

创建一个新的HTML邮件模板。

**端点**
```
POST /api/admin/email/templates
```

**请求体**
```json
{
  "name": "注册验证邮件",
  "subject": "欢迎注册 {{platformName}}",
  "htmlContent": "<html><body><h1>您好，{{username}}！</h1><p>您的验证码是：<strong>{{code}}</strong></p><p>有效期5分钟。</p></body></html>"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | ✅ | 模板名称（唯一标识） |
| subject | string | ✅ | 邮件主题，可包含占位符 |
| htmlContent | string | ✅ | HTML邮件内容，可包含占位符 |

**占位符语法**
```
{{变量名}}
```
- 使用双大括号包裹变量名
- 变量名只能包含字母、数字、下划线
- 示例：`{{username}}`, `{{verificationCode}}`, `{{platform_name}}`

**响应 (200 OK)**
```json
{
  "success": true,
  "templateId": 1
}
```

**错误响应**
```json
{
  "error": "Failed to create template"
}
```

---

### 2. 获取所有模板

获取所有邮件模板列表。

**端点**
```
GET /api/admin/email/templates
```

**响应 (200 OK)**
```json
[
  {
    "id": 1,
    "name": "注册验证邮件",
    "subject": "欢迎注册 {{platformName}}",
    "htmlContent": "<html>...</html>",
    "createdAt": "2025-01-01T00:00:00Z",
    "updatedAt": "2025-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "密码重置邮件",
    "subject": "重置密码 - {{platformName}}",
    "htmlContent": "<html>...</html>",
    "createdAt": "2025-01-02T00:00:00Z",
    "updatedAt": "2025-01-02T00:00:00Z"
  }
]
```

---

### 3. 获取单个模板

根据ID获取指定邮件模板。

**端点**
```
GET /api/admin/email/templates/{id}
```

**路径参数**
| 参数 | 类型 | 说明 |
|------|------|------|
| id | integer | 模板ID |

**响应 (200 OK)**
```json
{
  "id": 1,
  "name": "注册验证邮件",
  "subject": "欢迎注册 {{platformName}}",
  "htmlContent": "<html><body><h1>您好，{{username}}！</h1>...</body></html>",
  "createdAt": "2025-01-01T00:00:00Z",
  "updatedAt": "2025-01-01T00:00:00Z"
}
```

**错误响应 (404 Not Found)**
```json
{
  "error": "Template not found"
}
```

---

### 4. 更新邮件模板

更新指定的邮件模板。

**端点**
```
PUT /api/admin/email/templates/{id}
```

**路径参数**
| 参数 | 类型 | 说明 |
|------|------|------|
| id | integer | 模板ID |

**请求体**
```json
{
  "name": "注册验证邮件-v2",
  "subject": "欢迎注册 {{platformName}} 🎉",
  "htmlContent": "<html><body><h1>您好，{{username}}！</h1><p>更新后的内容...</p></body></html>"
}
```

**响应 (200 OK)**
```json
{
  "success": true
}
```

---

### 5. 删除邮件模板

删除指定的邮件模板。

**端点**
```
DELETE /api/admin/email/templates/{id}
```

**路径参数**
| 参数 | 类型 | 说明 |
|------|------|------|
| id | integer | 模板ID |

**响应 (200 OK)**
```json
{
  "success": true
}
```

---

## 邮件任务管理

### 1. 创建邮件发送任务

创建一个批量邮件发送任务，系统将异步执行。

**端点**
```
POST /api/admin/email/tasks
```

**请求体**
```json
{
  "templateId": 1,
  "variables": {
    "platformName": "NexTrader",
    "username": "张三",
    "code": "123456"
  },
  "recipients": [
    "user1@example.com",
    "user2@example.com",
    "user3@example.com"
  ]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| templateId | integer | ✅ | 要使用的模板ID |
| variables | object | ✅ | 占位符变量的键值对映射 |
| recipients | array | ✅ | 收件人邮箱地址列表 |

**变量替换规则**
- 模板中的所有`{{key}}`会被替换为`variables.key`的值
- 如果变量不存在，占位符将保持原样
- 变量值会应用到所有收件人的邮件中

**响应 (200 OK)**
```json
{
  "success": true,
  "taskId": 10
}
```

**任务执行流程**
1. 创建任务后立即返回任务ID
2. 后台异步开始发送邮件
3. 每封邮件间隔 500ms 发送（避免被封号）
4. 记录每封邮件的发送结果到日志表
5. 完成后更新任务状态和统计信息

**错误响应**
```json
// 模板不存在
{
  "error": "Template not found"
}

// 收件人为空
{
  "error": "Recipients cannot be empty"
}

// 变量格式错误
{
  "error": "Invalid variables format"
}
```

---

### 2. 获取所有任务

获取所有邮件发送任务列表。

**端点**
```
GET /api/admin/email/tasks
```

**响应 (200 OK)**
```json
[
  {
    "id": 10,
    "templateId": 1,
    "variables": "{\"platformName\":\"NexTrader\",\"code\":\"123456\"}",
    "recipients": "[\"user1@example.com\",\"user2@example.com\"]",
    "status": "completed",
    "createdAt": "2025-01-10T10:00:00Z",
    "startedAt": "2025-01-10T10:00:01Z",
    "completedAt": "2025-01-10T10:00:15Z",
    "totalCount": 2,
    "successCount": 2,
    "failedCount": 0
  },
  {
    "id": 9,
    "templateId": 2,
    "variables": "{\"username\":\"李四\"}",
    "recipients": "[\"user3@example.com\"]",
    "status": "processing",
    "createdAt": "2025-01-10T09:00:00Z",
    "startedAt": "2025-01-10T09:00:01Z",
    "completedAt": null,
    "totalCount": 1,
    "successCount": 0,
    "failedCount": 0
  }
]
```

**任务状态说明**
- `pending`: 待执行
- `processing`: 执行中
- `completed`: 已完成（可能包含部分失败）
- `failed`: 任务失败（模板加载失败等）

---

### 3. 获取任务详情

获取指定任务的详细信息，包括所有发送日志。

**端点**
```
GET /api/admin/email/tasks/{id}
```

**路径参数**
| 参数 | 类型 | 说明 |
|------|------|------|
| id | integer | 任务ID |

**响应 (200 OK)**
```json
{
  "id": 10,
  "templateId": 1,
  "variables": "{\"platformName\":\"NexTrader\",\"code\":\"123456\"}",
  "recipients": "[\"user1@example.com\",\"user2@example.com\",\"user3@example.com\"]",
  "status": "completed",
  "createdAt": "2025-01-10T10:00:00Z",
  "startedAt": "2025-01-10T10:00:01Z",
  "completedAt": "2025-01-10T10:00:20Z",
  "totalCount": 3,
  "successCount": 2,
  "failedCount": 1,
  "logs": [
    {
      "id": 1,
      "taskId": 10,
      "recipient": "user1@example.com",
      "status": "success",
      "errorMessage": null,
      "sentAt": "2025-01-10T10:00:05Z"
    },
    {
      "id": 2,
      "taskId": 10,
      "recipient": "user2@example.com",
      "status": "success",
      "errorMessage": null,
      "sentAt": "2025-01-10T10:00:10Z"
    },
    {
      "id": 3,
      "taskId": 10,
      "recipient": "user3@example.com",
      "status": "failed",
      "errorMessage": "Failed to send email: Invalid recipient email: ...",
      "sentAt": "2025-01-10T10:00:15Z"
    }
  ]
}
```

**错误响应 (404 Not Found)**
```json
{
  "error": "Task not found"
}
```

---

## 使用流程

### 完整示例：发送注册验证邮件

#### 步骤 1: 创建模板

```http
POST /api/admin/email/templates
Authorization: Bearer <admin-token>
Content-Type: application/json

{
  "name": "用户注册验证",
  "subject": "【{{platformName}}】验证您的邮箱",
  "htmlContent": "<!DOCTYPE html><html><head><style>body{font-family:Arial,sans-serif;background-color:#f4f4f4;padding:20px}.container{background:white;padding:30px;border-radius:8px;max-width:600px;margin:0 auto}.code{font-size:32px;color:#4CAF50;font-weight:bold;letter-spacing:5px;padding:20px;background:#f9f9f9;border-radius:4px;text-align:center;margin:20px 0}.footer{color:#666;font-size:12px;margin-top:30px;border-top:1px solid #eee;padding-top:20px}</style></head><body><div class='container'><h1>欢迎注册 {{platformName}}！</h1><p>尊敬的 <strong>{{username}}</strong>，</p><p>感谢您注册我们的平台。请使用以下验证码完成注册：</p><div class='code'>{{verificationCode}}</div><p><strong>注意事项：</strong></p><ul><li>验证码有效期为 <strong>5分钟</strong></li><li>请勿将验证码泄露给他人</li><li>如非本人操作，请忽略此邮件</li></ul><div class='footer'><p>此邮件由系统自动发送，请勿回复。</p><p>&copy; 2025 {{platformName}}. All rights reserved.</p></div></div></body></html>"
}
```

**响应：**
```json
{
  "success": true,
  "templateId": 1
}
```

#### 步骤 2: 发送邮件

```http
POST /api/admin/email/tasks
Authorization: Bearer <admin-token>
Content-Type: application/json

{
  "templateId": 1,
  "variables": {
    "platformName": "NexTrader",
    "username": "张三",
    "verificationCode": "AB3F92"
  },
  "recipients": [
    "zhangsan@example.com"
  ]
}
```

**响应：**
```json
{
  "success": true,
  "taskId": 15
}
```

#### 步骤 3: 查询任务状态

```http
GET /api/admin/email/tasks/15
Authorization: Bearer <admin-token>
```

**响应：**
```json
{
  "id": 15,
  "templateId": 1,
  "variables": "{\"platformName\":\"NexTrader\",\"username\":\"张三\",\"verificationCode\":\"AB3F92\"}",
  "recipients": "[\"zhangsan@example.com\"]",
  "status": "completed",
  "createdAt": "2025-01-10T15:30:00Z",
  "startedAt": "2025-01-10T15:30:01Z",
  "completedAt": "2025-01-10T15:30:05Z",
  "totalCount": 1,
  "successCount": 1,
  "failedCount": 0,
  "logs": [
    {
      "id": 100,
      "taskId": 15,
      "recipient": "zhangsan@example.com",
      "status": "success",
      "errorMessage": null,
      "sentAt": "2025-01-10T15:30:03Z"
    }
  ]
}
```

---

### 批量发送示例

```http
POST /api/admin/email/tasks
Authorization: Bearer <admin-token>
Content-Type: application/json

{
  "templateId": 1,
  "variables": {
    "platformName": "NexTrader",
    "announcementTitle": "系统维护通知",
    "maintenanceTime": "2025-01-15 02:00-04:00"
  },
  "recipients": [
    "user1@example.com",
    "user2@example.com",
    "user3@example.com",
    "user4@example.com",
    "user5@example.com"
  ]
}
```

**执行特点：**
- ✅ 立即返回任务ID
- ✅ 后台异步发送，每封间隔500ms
- ✅ 总耗时约：5封 × 0.5秒 = 2.5秒
- ✅ 实时记录每封邮件的发送结果
- ✅ 可随时查询任务进度

---

## 错误处理

### HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 | 请求成功 |
| 400 | 请求参数错误 |
| 401 | 未授权（Token无效或缺失） |
| 403 | 禁止访问（非管理员） |
| 404 | 资源不存在 |
| 500 | 服务器内部错误 |

### 常见错误

**1. 模板名称重复**
```json
{
  "error": "Failed to create template"
}
```
解决：使用不同的模板名称

**2. 模板不存在**
```json
{
  "error": "Template not found"
}
```
解决：使用 GET /templates 查看可用模板

**3. 收件人列表为空**
```json
{
  "error": "Recipients cannot be empty"
}
```
解决：至少提供一个收件人邮箱

**4. SMTP 发送失败**
邮件日志中会记录详细错误信息：
```json
{
  "status": "failed",
  "errorMessage": "Failed to send email: SMTP error - Connection timeout"
}
```

---

## 最佳实践

### 1. 模板设计

✅ **推荐做法：**
- 使用语义化的变量名：`{{userName}}` 而非 `{{x}}`
- 提供完整的HTML结构，包括DOCTYPE和样式
- 使用内联CSS，避免外部样式表
- 测试多个邮件客户端的显示效果

❌ **避免：**
- 使用JavaScript（大部分邮件客户端不支持）
- 依赖外部资源（图片建议使用完整URL）
- 过于复杂的布局（可能导致渲染问题）

### 2. 批量发送

✅ **推荐做法：**
- 分批发送大量邮件（建议每批不超过100封）
- 在非高峰时段执行大批量任务
- 定期检查任务日志，处理失败情况

❌ **避免：**
- 一次性发送数千封邮件
- 短时间内创建多个大批量任务
- 忽略失败日志

### 3. 性能优化

- 当前发送间隔：500ms/封
- 100封邮件预计耗时：50秒
- 1000封邮件预计耗时：8.3分钟

**建议：**
- 对于超大批量（>1000封），考虑使用专业邮件服务
- 监控SMTP服务器的发送限额
- 定期清理历史任务和日志数据

---

## 环境配置

邮件发送使用 `.env` 文件中的SMTP配置：

```env
MAIL_HOST=smtp.gmail.com
MAIL_USER=your-email@gmail.com
MAIL_PASS=your-app-password
```

**Gmail 配置示例：**
1. 开启两步验证
2. 生成应用专用密码
3. 使用应用密码作为 `MAIL_PASS`

**其他SMTP服务商：**
- **腾讯企业邮箱**: `smtp.exmail.qq.com:465`
- **阿里云邮箱**: `smtp.aliyun.com:465`
- **网易邮箱**: `smtp.163.com:465`
- **Outlook**: `smtp.office365.com:587`

---

## 技术细节

### 占位符解析

使用正则表达式匹配：
```rust
r"\{\{(\w+)\}\}"
```

### 异步执行

使用 `tokio::spawn` 异步执行发送任务，不阻塞主线程：
```rust
tokio::spawn(async move {
    execute_email_task(task_id, db_clone, mail_config_clone).await;
});
```

### 发送限流

每封邮件发送后等待500ms：
```rust
tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
```

---

## 更新日志

### v1.0.0 (2025-01-10)
- ✨ 初始版本发布
- ✅ 邮件模板 CRUD
- ✅ 批量异步发送
- ✅ 详细日志记录
- ✅ 占位符变量替换

---

## 联系支持

如有问题或建议，请联系技术支持团队。

---

**文档版本**: v1.0.0
**最后更新**: 2025-01-10
**适用系统版本**: NexTrader Backend v0.1.0
