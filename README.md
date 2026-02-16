# X-Photo

一个本地优先的照片管理控制台（V1），用于快速完成：照片扫描、搜索、相册浏览、收藏管理、任务观测与问题排查。

> 当前定位：工程化可用的 V1 管理端（非最终产品 UI）。

## 功能概览

- 照片：按时间瀑布流浏览，支持关键词与字段化搜索
- 收藏：收藏/取消收藏、时间线展示、快速取消收藏
- 相册：相册列表与相册内照片筛选
- 设置：API Base、健康检查、Source 创建与扫描触发
- 状态：任务总览、任务列表、任务详情、取消/重试

## 技术栈

- 后端：Rust + Axum + SQLx（SQLite）
- 前端：原生 HTML/CSS/JS（零构建）

## 快速开始

### 1) 拉取代码

```bash
git clone <your-repo-url> x_photo
cd x_photo
```

### 2) 一键启动（推荐）

- Linux/macOS

```bash
./start.sh
```

- Windows PowerShell

```powershell
.\start.ps1
```

- Windows CMD

```bat
start.bat
```

默认地址：

- Web: `http://127.0.0.1:5174`
- API: `http://127.0.0.1:8080/rpc/v1`

### 3) 停止服务

- Linux/macOS: `./stop.sh`
- Windows PowerShell: `.\stop.ps1`
- Windows CMD: `stop.bat`

## 常用文档

- 完整指南（开发/测试/联调）：[`doc/快速开始与使用指南.md`](doc/快速开始与使用指南.md)
- 简版手册（面向非开发用户）：[`doc/用户操作手册（简版）.md`](doc/用户操作手册（简版）.md)
- Web 控制台说明：[`web/README.md`](web/README.md)
- 后端 OpenAPI：[`doc/OpenAPI.yaml`](doc/OpenAPI.yaml)
- 任务系统设计：[`doc/任务系统设计.md`](doc/任务系统设计.md)
- 日志规范：[`doc/日志规范.md`](doc/日志规范.md)

## 仓库结构

```text
x_photo/
├─ service/          # Rust 后端服务
├─ web/              # 前端控制台（静态文件）
├─ doc/              # 设计文档、使用手册、OpenAPI
├─ start.sh/.ps1     # 一键启动脚本
├─ stop.sh/.ps1      # 一键停止脚本
└─ README.md
```

## 运行与安全说明

- 默认仅监听 `127.0.0.1`（本机访问）
- 默认运行数据目录为 `~/.xphoto`
- 如需外网访问，请先补齐认证与安全策略，再调整绑定地址

## 反馈与协作

- 欢迎通过 Issue/PR 提交问题与改进建议
- 建议附带复现步骤、日志片段与环境信息（OS、Rust/Python 版本）
