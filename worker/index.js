/**
 * 欢迎使用此 Cloudflare Worker 脚本
 *
 * 此脚本旨在反向代理来自 https://chaincatcher.com/rss/clist 的 RSS feed。
 *
 * 主要功能:
 * 1.  **路径特定**: 只会响应对 `/rss/clist` 路径的请求。
 * 2.  **反向代理**: 从源站 `chaincatcher.com` 获取内容并返回给用户。
 * 3.  **缓存机制**: 对成功获取的 RSS feed 进行10分钟的缓存，以减少对源站的请求并提高速度。
 * 4.  **跨域支持 (CORS)**: 包含必要的 CORS 头，允许任何域名下的前端应用访问此 feed。
 * 5.  **处理 OPTIONS 请求**: 正确响应浏览器的 CORS 预检请求。
 */

// 定义源站和目标路径
const TARGET_HOST = "chaincatcher.com";
const TARGET_PATH = "/rss/clist";

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    // 预设 CORS 头，以便在所有响应中重复使用
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*', // 允许任何来源
      'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS', // 允许的请求方法
      'Access-Control-Allow-Headers': 'Content-Type', // 允许的请求头
      'Content-Type': 'application/rss+xml; charset=utf-8', // 设置正确的RSS内容类型
    };

    // 首先，处理浏览器为检查CORS策略而发送的 OPTIONS 预检请求
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        status: 204,
        headers: corsHeaders,
      });
    }

    // 检查请求的路径是否是我们想要代理的路径
    if (url.pathname !== TARGET_PATH) {
      return new Response('路径未找到。此 Worker 仅代理 /rss/clist', {
        status: 404,
        headers: {
          ...corsHeaders,
          'Content-Type': 'text/plain; charset=utf-8',
        },
      });
    }

    // 构建到源站的请求URL
    const destinationURL = `https://${TARGET_HOST}${TARGET_PATH}`;

    // 使用 Cloudflare 的 fetch API 进行请求
    // 通过 `cf` 对象来配置缓存
    const response = await fetch(destinationURL, {
      method: 'GET',
      headers: {
        'User-Agent': 'Cloudflare-Worker-RSS-Proxy/1.0',
        'Accept': 'application/rss+xml, application/xml',
      },
      cf: {
        // 关键：设置 Cloudflare CDN 的缓存时间和类型
        // cacheTtl: 缓存时间（秒）
        // cacheEverything: 强制缓存所有内容，即使源站响应头建议不缓存
        cacheTtl: 600, // 缓存300秒（6分钟）
        cacheEverything: true,
      },
    });

    // 如果源站返回的不是成功状态码，则直接透传其响应
    if (!response.ok) {
      console.error(`源站错误: ${response.status} ${response.statusText}`);
      return new Response(`无法从源站获取内容。状态码: ${response.status}`, {
        status: response.status,
        headers: {
          ...corsHeaders,
          'Content-Type': 'text/plain; charset=utf-8',
        },
      });
    }

    // 创建一个新的可变响应，以便我们可以添加自定义的头
    // response.clone() 是必需的，因为一个响应体只能被读取一次
    const newResponse = new Response(response.body, response);

    // 添加/覆盖 CORS 和缓存相关的头
    for (const [key, value] of Object.entries(corsHeaders)) {
      newResponse.headers.set(key, value);
    }
    
    // （可选）设置浏览器缓存头，让客户端也缓存10分钟
    newResponse.headers.set('Cache-Control', 'public, max-age=600');
    
    // 添加一个自定义头，方便调试，以确认响应是否来自我们的 Worker
    newResponse.headers.set('X-Worker-Proxy', 'Active');
    
    // 返回最终的响应
    return newResponse;
  },
};