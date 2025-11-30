/**
 * Cloudflare Worker - 加密货币新闻 RSS 代理
 *
 * 支持的路径:
 * 1.  `/rss/clist` - 代理 ChainCatcher RSS feed (中文)
 * 2.  `/rss/hybrid` - 使用 Followin API 生成中英双语 RSS feed
 *
 * 主要功能:
 * - 缓存机制: 10分钟缓存，减少对源站的请求
 * - 跨域支持 (CORS): 允许任何域名访问
 * - 处理 OPTIONS 预检请求
 */

// ChainCatcher 配置
const CHAINCATCHER_HOST = "chaincatcher.com";
const CHAINCATCHER_PATH = "/rss/clist";

// Followin API 配置
const FOLLOWIN_API = "https://api.followin.io/feed/list/recommended/news";
const DEFAULT_PAGES = 20; // 默认拉取20页，共200条新闻
const MAX_PAGES = 30;     // 最大允许拉取30页，共300条新闻

// 通用 CORS 头
const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
};

/**
 * 将 Followin API 数据转换为 RSS XML 格式
 * 每篇文章拆分为中英文两条独立的 item
 * @param {Array} allItems - 合并后的所有新闻列表
 */
function generateHybridRSS(allItems) {
  const items = allItems.flatMap(item => {
    const pubDate = new Date(item.publish_time).toUTCString();
    const tagsHtml = item.tags && item.tags.length > 0 
      ? `<p><strong>Tags:</strong> ${item.tags.map(t => `${t.name} (${t.symbol}) ${t.price} ${t.percent_change_24h}`).join(', ')}</p>` 
      : '';
    
    // 英文版 item
    const enItem = `
    <item>
      <title><![CDATA[${item.translated_title || item.title}]]></title>
      <link>${escapeXml(item.source_url)}</link>
      <description><![CDATA[${escapeXml(item.translated_content || item.content)}]]></description>
      <content:encoded><![CDATA[<p>${escapeXml(item.translated_content || item.content)}</p>${tagsHtml}]]></content:encoded>
      <pubDate>${pubDate}</pubDate>
      <guid isPermaLink="false">followin-${item.id}-en</guid>
      <language>en-us</language>
      <source url="https://followin.io">${escapeXml(item.nickname)}</source>
      <category>${item.important ? 'Important' : 'News'}</category>
      <dc:creator><![CDATA[${item.nickname}]]></dc:creator>
    </item>`;

    // 中文版 item
    const cnItem = `
    <item>
      <title><![CDATA[${item.title}]]></title>
      <link>${escapeXml(item.source_url)}</link>
      <description><![CDATA[${escapeXml(item.content)}]]></description>
      <content:encoded><![CDATA[<p>${escapeXml(item.content)}</p>${tagsHtml}]]></content:encoded>
      <pubDate>${pubDate}</pubDate>
      <guid isPermaLink="false">followin-${item.id}-zh</guid>
      <language>zh-cn</language>
      <source url="https://followin.io">${escapeXml(item.nickname)}</source>
      <category>${item.important ? 'Important' : 'News'}</category>
      <dc:creator><![CDATA[${item.nickname}]]></dc:creator>
    </item>`;

    return [enItem, cnItem];
  }).join('\n');

  const now = new Date().toUTCString();
  
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:dc="http://purl.org/dc/elements/1.1/" version="2.0">
  <channel>
    <title>Crypto News Hybrid Feed (EN/CN)</title>
    <link>https://followin.io</link>
    <description>Aggregated cryptocurrency news feed with bilingual support. Each article has both English (en-us) and Chinese (zh-cn) versions.</description>
    <language>en-us</language>
    <lastBuildDate>${now}</lastBuildDate>
    <generator>Cloudflare Worker RSS Proxy</generator>
    <image>
      <title>Crypto News Hybrid Feed</title>
      <url>https://static.fwimg.io/img/logo.png</url>
      <link>https://followin.io</link>
    </image>
${items}
  </channel>
</rss>`;
}

/**
 * 转义 XML 特殊字符
 */
function escapeXml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/**
 * 处理 ChainCatcher RSS 代理请求
 */
async function handleChainCatcher() {
  const destinationURL = `https://${CHAINCATCHER_HOST}${CHAINCATCHER_PATH}`;
  
  const response = await fetch(destinationURL, {
    method: 'GET',
    headers: {
      'User-Agent': 'Cloudflare-Worker-RSS-Proxy/1.0',
      'Accept': 'application/rss+xml, application/xml',
    },
    cf: {
      cacheTtl: 600,
      cacheEverything: true,
    },
  });

  if (!response.ok) {
    throw new Error(`ChainCatcher 源站错误: ${response.status}`);
  }

  return new Response(response.body, {
    status: 200,
    headers: {
      ...corsHeaders,
      'Content-Type': 'application/rss+xml; charset=utf-8',
      'Cache-Control': 'public, max-age=600',
      'X-Worker-Proxy': 'ChainCatcher',
    },
  });
}

/**
 * 从 Followin API 拉取多页数据
 * @param {number} pages - 要拉取的页数
 * @returns {Promise<Array>} - 合并后的新闻列表
 */
async function fetchFollowinPages(pages) {
  const allItems = [];
  let lastCursor = '';
  
  for (let i = 0; i < pages; i++) {
    const body = {
      only_important: false,
      no_ad: true,
      subgroups: [],
    };
    
    if (lastCursor) {
      body.last_cursor = lastCursor;
    }
    
    const response = await fetch(FOLLOWIN_API, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'User-Agent': 'Cloudflare-Worker-RSS-Proxy/1.0',
      },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      throw new Error(`Followin API 错误: ${response.status}`);
    }

    const data = await response.json();
    
    if (data.code !== 2000 || !data.data?.list) {
      throw new Error(`Followin API 返回异常: ${data.msg}`);
    }

    allItems.push(...data.data.list);
    
    // 检查是否还有更多数据
    if (!data.data.has_more) {
      break;
    }
    
    lastCursor = data.data.last_cursor;
  }
  
  return allItems;
}

/**
 * 处理 Followin Hybrid RSS 请求
 * 支持 ?pages=N 参数指定拉取页数 (1-10)
 */
async function handleHybrid(url) {
  // 从 URL 参数获取页数，默认5页
  const pagesParam = url.searchParams.get('pages');
  let pages = pagesParam ? parseInt(pagesParam, 10) : DEFAULT_PAGES;
  
  // 限制页数范围
  if (isNaN(pages) || pages < 1) pages = 1;
  if (pages > MAX_PAGES) pages = MAX_PAGES;
  
  const allItems = await fetchFollowinPages(pages);
  const rssXml = generateHybridRSS(allItems);

  return new Response(rssXml, {
    status: 200,
    headers: {
      ...corsHeaders,
      'Content-Type': 'application/rss+xml; charset=utf-8',
      'Cache-Control': 'public, max-age=600',
      'X-Worker-Proxy': 'Followin-Hybrid',
      'X-Items-Count': String(allItems.length),
      'X-Pages-Fetched': String(pages),
    },
  });
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    // 处理 CORS 预检请求
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        status: 204,
        headers: corsHeaders,
      });
    }

    try {
      // 路由分发
      switch (url.pathname) {
        case '/rss/clist':
          return await handleChainCatcher();
        
        case '/rss/hybrid':
          return await handleHybrid(url);
        
        default:
          return new Response(
            `路径未找到。支持的路径:\n- /rss/clist (ChainCatcher 中文)\n- /rss/hybrid (Followin 中英双语，默认200条)\n- /rss/hybrid?pages=N (指定页数，1-30页，每页10条)`,
            {
              status: 404,
              headers: {
                ...corsHeaders,
                'Content-Type': 'text/plain; charset=utf-8',
              },
            }
          );
      }
    } catch (error) {
      console.error(`Worker 错误: ${error.message}`);
      return new Response(`服务错误: ${error.message}`, {
        status: 500,
        headers: {
          ...corsHeaders,
          'Content-Type': 'text/plain; charset=utf-8',
        },
      });
    }
  },
};