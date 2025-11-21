# 任务系统 API 文档 (Task System API Documentation)

本文档详细描述了任务系统的前端接口（用户端）和管理后台接口（管理员端）。

## 1. 用户端接口 (Client-Side API)

**Base URL**: `/api/mission`

### 1.1 获取任务列表 (Get Task List)

获取当前用户的所有可用任务及其状态。

- **Endpoint**: `/list`
- **Method**: `GET`
- **Auth**: Required (Bearer Token)

**Response (Success 200)**:
```json
[
  {
    "id": 1,
    "name": "新手注册奖励",
    "description": "完成账号注册并验证邮箱",
    "reward_amount": 100.0,
    "task_type": "REGISTER",
    "condition_value": 1,
    "is_daily": false,
    "is_active": true,
    "status": "COMPLETED", // 状态: NOT_STARTED, IN_PROGRESS, COMPLETED, CLAIMED
    "progress": 1          // 当前进度
  },
  {
    "id": 2,
    "name": "邀请5位好友",
    "description": "邀请5个一级下线完成注册",
    "reward_amount": 500.0,
    "task_type": "REFERRAL_COUNT",
    "condition_value": 5,
    "is_daily": false,
    "is_active": true,
    "status": "IN_PROGRESS",
    "progress": 2
  }
]
```

**状态说明 (Status)**:
- `NOT_STARTED`: 任务未开始或未满足条件。
- `IN_PROGRESS`: 任务进行中（例如邀请了2/5人）。
- `COMPLETED`: 任务已完成，可以领取奖励。
- `CLAIMED`: 奖励已领取。

---

### 1.2 领取奖励 (Claim Reward)

领取已完成任务的奖励。

- **Endpoint**: `/claim`
- **Method**: `POST`
- **Auth**: Required (Bearer Token)
- **Content-Type**: `application/json`

**Request Body**:
```json
{
  "task_id": 1
}
```

**Response (Success 200)**:
```json
{
  "message": "领取成功",
  "reward": 100.0
}
```

**Error Responses**:
- `400 Bad Request`: "任务不存在" 或 "任务未完成或已领取"

---

### 1.3 上报行为 (Report Action)

用于前端触发某些需要手动上报的任务（如分享、观看直播）。

- **Endpoint**: `/action`
- **Method**: `POST`
- **Auth**: Required (Bearer Token)
- **Content-Type**: `application/json`

**Request Body**:
```json
{
  "action": "daily_share" // 可选值: "daily_live", "daily_share"
}
```

**Response (Success 200)**:
```json
{
  "message": "行为已记录"
}
```

---

## 2. 任务类型与逻辑说明 (Task Types & Logic)

前端开发需注意不同 `task_type` 的判定逻辑：

| 任务类型 (`task_type`) | 描述 | 判定逻辑 (Backend Logic) | `condition_value` 作用 |
| :--- | :--- | :--- | :--- |
| **REGISTER** | 注册 | 用户存在即视为完成。 | **忽略** (填1) |
| **BIND_EXCHANGE** | 绑定交易所 | 检查用户是否绑定了至少 1 个交易所 API Key。 | **忽略** (填1) |
| **REFERRAL_COUNT** | 邀请人数 | 统计用户的一级下线（直推）人数。 | **生效** (目标人数) |
| **TEAM_SIZE** | 团队人数 | 统计用户的整个裂变团队（无限层级）总人数。 | **生效** (目标人数) |
| **DAILY_LIVE** | 每日直播 | 需前端调用 `/action` 接口上报 `daily_live`。 | **忽略** (填1) |
| **DAILY_SHARE** | 每日分享 | 需前端调用 `/action` 接口上报 `daily_share`。 | **忽略** (填1) |
| **TRADE_ACTIVITY** | 交易活跃 | 检查用户或其直推下线在**昨天**是否有交易记录。 | **忽略** (填1) |

**注意**:
- `is_daily` (每日任务): 如果为 `true`，任务状态会在每天 UTC 0点重置。用户每天都可以完成并领取一次。
- `condition_value`: 仅在 `REFERRAL_COUNT` 和 `TEAM_SIZE` 中作为具体的数字门槛，其他类型中仅作为占位符（建议填1）。

---

## 3. 管理员接口 (Admin API)

**Base URL**: `/api/admin/tasks`

### 3.1 获取所有任务
- **Method**: `GET`
- **Response**: 任务对象数组。

### 3.2 创建任务
- **Method**: `POST`
- **Body**:
```json
{
  "name": "任务名称",
  "description": "描述",
  "rewardAmount": 100,
  "taskType": "REGISTER",
  "conditionValue": 1,
  "isDaily": false,
  "isActive": true
}
```

### 3.3 更新任务
- **Method**: `PUT`
- **URL**: `/api/admin/tasks/{id}`
- **Body**: 同创建任务。

### 3.4 删除任务
- **Method**: `DELETE`
- **URL**: `/api/admin/tasks/{id}`
