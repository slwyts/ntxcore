const winston = require('winston');
// 引入每日分割插件
require('winston-daily-rotate-file');
const fs = require('fs');
const path = require('path');

const logDir = 'logs'; // 定义日志文件存放目录

// 如果 'logs' 目录不存在，则创建它
if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir);
}

// 创建一个每日分割的 transport
const dailyRotateFileTransport = new winston.transports.DailyRotateFile({
    filename: path.join(logDir, '%DATE%-service.log'), // 文件名格式
    datePattern: 'YYYY-MM-DD',                         // 日期格式，每天一个文件
    zippedArchive: true,                               // 自动压缩旧的日志文件
    maxSize: '20m',                                    // 单个文件最大20MB
    maxFiles: '14d',                                   // 最多保留14天的日志
    format: winston.format.combine(
        winston.format.printf(info => `[${info.timestamp}] [${info.level.toUpperCase()}] ${info.message}`)
    )
});

// 创建 logger 实例
const logger = winston.createLogger({
    // 设置日志级别，只有 'info' 或更高级别的日志才会被记录
    level: 'info',
    // 设置日志格式
    format: winston.format.combine(
        winston.format.timestamp({
            format: 'YYYY-MM-DD HH:mm:ss'
        }),
        // 定义纯文本格式
        winston.format.simple() 
    ),
    // 定义日志输出目标 (transports)
    transports: [
        // 1. 输出到控制台
        new winston.transports.Console({
            format: winston.format.combine(
                winston.format.colorize(), // 为控制台输出添加颜色
                winston.format.printf(info => `[${info.timestamp}] [${info.level}] ${info.message}`)
            )
        }),
        // 2. 输出到文件 (使用我们上面定义的每日分割)
        dailyRotateFileTransport
    ]
});

// 导出 logger 实例
module.exports = logger;