## NextTradeDAO CEX返佣平台
### 课程与支付系统API文档

#### 核心概念

  * **课程 (Course)**: 具体的学习内容，例如一篇文章或一个模型。课程本身不直接售卖，而是被打包进**权限组**。
  * **权限组 (Permission Group)**: 一个或多个课程的集合。用户通过购买**课程套餐**来获得某个权限组的访问权。
  * **课程套餐 (Course Package)**: 权限组的购买选项，定义了特定权限组的有效时长和价格。
  * **订单 (Order)**: 用户购买课程套餐时生成的支付凭据。

-----

### **一、 课程相关API (面向普通用户)**

这些API允许用户查看课程和套餐信息。

#### 1\. 获取所有课程列表 (并标记用户解锁状态)

  * **Endpoint**: `GET /api/courses/all`

  * **认证**: 可选 (如果提供Token，会返回当前用户的解锁状态)

  * **功能**: 获取平台上所有可用的课程，并根据用户的权限状态，标记每个课程是否已解锁。对于未解锁的课程，`content`字段将为空。

  * **返回信息解释**:

      * `isUnlocked`: `true` 表示用户有权访问该课程，`false` 表示无权访问。
      * `requiredGroups`: 解锁此课程所需要的权限组列表。
      * `course_type`: 课程类型，根据您的补充，其对应关系如下：
          * `article`: 学习资源
          * `dark_horse`: 黑马模型
          * `signal`: 策略信号
          * `loop_comm`: Loop社区

    <!-- end list -->

    ```json
    // 成功响应示例 (用户已登录)
    [
        {
            "id": 101,
            "course_type": "dark_horse", // 黑马模型
            "name": "第一期黑马模型详解",
            "description": "深入理解黑马模型的核心逻辑与应用。",
            "content": "这里是课程的具体内容...", // 因为 isUnlocked 为 true，所以有内容
            "isUnlocked": true,
            "required_groups": [
                {
                    "id": 2,
                    "name": "VIP会员"
                }
            ]
        },
        {
            "id": 102,
            "course_type": "signal", // 策略信号
            "name": "ETH 高频交易信号",
            "description": "基于链上数据的ETH交易信号。",
            "content": "", // 因为 isUnlocked 为 false，所以内容为空
            "isUnlocked": false,
            "required_groups": [
                {
                    "id": 3,
                    "name": "信号订阅"
                }
            ]
        }
    ]
    ```

#### 2\. 获取所有权限组及套餐

  * **Endpoint**: `GET /api/courses/permission_groups`

  * **认证**: 无需

  * **功能**: 获取所有可购买的权限组，以及每个权限组下包含的具体套餐（价格、时长等）。

  * **返回信息解释**:

      * `group`: 权限组的基本信息。
      * `packages`: 该权限组下可供购买的套餐列表。

    <!-- end list -->

    ```json
    // 成功响应示例
    [
        {
            "group": {
                "id": 2,
                "name": "VIP会员",
                "created_at": "2023-10-27T10:00:00Z"
            },
            "packages": [
                {
                    "id": 1,
                    "group_id": 2,
                    "duration_days": 30,
                    "price": 100.0,
                    "currency": "USDT"
                },
                {
                    "id": 2,
                    "group_id": 2,
                    "duration_days": 365,
                    "price": 999.0,
                    "currency": "USDT"
                }
            ]
        }
    ]
    ```

#### 3\. 获取我购买的课程

  * **Endpoint**: `GET /api/user/my_courses`
  * **认证**: **必须**
  * **功能**: 获取当前登录用户已拥有访问权限的所有课程列表。
  * **返回信息解释**: 返回一个课程对象的数组，结构与 `GET /api/courses/all` 中的单个课程对象相同，但只包含已解锁的课程。

-----

### **二、 支付相关API (面向普通用户)**

#### 1\. 创建订单

  * **Endpoint**: `POST /api/payment/orders`

  * **认证**: **必须**

  * **功能**: 用户选择一个**课程套餐**后，调用此API创建支付订单。

  * **请求体**:

    ```json
    {
        "package_id": 2 // 要购买的课程套餐ID
    }
    ```

  * **返回信息解释**:

      * `orderId`: 订单的唯一ID。
      * `amount`: 套餐的原始价格。
      * `paymentAmount`: **用户需要实际支付的金额**。这是一个在原始价格基础上增加了一个微小随机数（0.00001到0.00999之间）的唯一金额，用于在链上区分不同用户的支付。
      * `paymentAddress`: 平台收款的钱包地址（从环境变量读取）。

    <!-- end list -->

    ```json
    // 成功响应示例
    {
        "message": "订单创建成功，请支付",
        "orderId": 123,
        "amount": 999.0,
        "paymentAmount": 999.00123, // 用户需精确支付此金额
        "currency": "USDT",
        "paymentAddress": "0xYourWalletAddress..."
    }
    ```

#### 2\. 获取我的订单列表

  * **Endpoint**: `GET /api/payment/orders`

  * **认证**: **必须**

  * **功能**: 获取当前登录用户的所有历史订单。

  * **返回信息解释**: 返回一个订单对象的数组。

      * `status`: 订单状态，`pending` (待支付), `confirmed` (已确认), `closed` (已关闭)。

    <!-- end list -->

    ```json
    // 成功响应示例
    [
        {
            "id": 123,
            "user_id": 45,
            "package_id": 2,
            "amount": 999.0,
            "paymentAmount": 999.00123,
            "currency": "USDT",
            "status": "confirmed",
            "created_at": "2023-10-27T11:00:00Z",
            "updated_at": "2023-10-27T11:05:00Z"
        },
        {
            "id": 122,
            "user_id": 45,
            "package_id": 1,
            "amount": 100.0,
            "paymentAmount": 100.00456,
            "currency": "USDT",
            "status": "pending",
            "created_at": "2023-10-26T15:30:00Z",
            "updated_at": "2023-10-26T15:30:00Z"
        }
    ]
    ```

-----

### **三、 管理后台API (面向管理员)**

所有管理API都需要在请求头中提供有效的管理员Token或API Key进行认证。

#### 1\. 课程管理

  * `GET /api/admin/courses/all`: 获取所有课程（无论是否显示）。
  * `POST /api/admin/courses`: 创建一个新课程。
  * `PUT /api/admin/courses/{id}`: 更新指定ID的课程。
  * `DELETE /api/admin/courses/{id}`: 删除指定ID的课程。
  * `POST /api/admin/courses/{course_id}/assign_group`: 为指定课程分配一个权限组。
  * `GET /api/admin/courses/{id}/groups`: 获取课程关联的所有权限组。

#### 2\. 权限组管理

  * `GET /api/admin/permission_groups/all`: 获取所有权限组。
  * `POST /api/admin/permission_groups`: 创建一个新权限组。
  * `PUT /api/admin/permission_groups/{id}`: 更新指定ID的权限组。
  * `DELETE /api/admin/permission_groups/{id}`: 删除指定ID的权限组。

#### 3\. 课程套餐管理

  * `GET /api/admin/course_packages/all`: 获取所有课程套餐。
  * `POST /api/admin/course_packages`: 创建一个新套餐。
  * `PUT /api/admin/course_packages/{id}`: 更新指定ID的套餐。
  * `DELETE /api/admin/course_packages/{id}`: 删除指定ID的套餐。

#### 4\. 订单与权限管理

  * `GET /api/admin/orders/all`: 获取所有用户订单，可通过 `status` 查询参数筛选。
  * `POST /api/admin/orders/{order_id}/confirm`: **手动确认**指定ID的订单已支付。此操作会将订单状态置为`confirmed`，并为用户授予相应权限。
  * `GET /api/admin/users/{user_id}/permissions`: 查看指定用户的权限列表。
  * `POST /api/admin/users/{user_id}/grant_permission`: 手动为用户授予某个权限组的访问权。
  * `POST /api/admin/users/{user_id}/revoke_permission`: 手动移除用户的某个权限。
