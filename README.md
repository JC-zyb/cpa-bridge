# cpa-bridge

一个小型桌面工具，用来把本地 CPA 账号 JSON 转成 Sub2Api 可导入格式，并直接上传到 Sub2Api。

## 功能

- 选择单个 JSON 文件或目录
- 预览转换结果
- 导出总文件
- 检查 Sub2Api 连接
- 推送到 Sub2Api
- 自动跳过已存在账号

## 运行

```powershell
npm install
npm run tauri:dev
```

## 构建

```powershell
npx tsc --noEmit
npm run build
npm run tauri:build
```

## 使用流程

1. 选择 **目录** 或 **单个 JSON 文件**
2. 点击 **开始预览**，查看当前数据能否正确转换
3. 在左侧预览区确认结果；如果需要留档，可以先点击 **导出总文件**
4. 在右侧填写：
   - Sub2Api 接口地址
   - 管理员邮箱
   - 管理员密码
5. 点击 **检查连接**，确认账号和地址可用
6. 点击 **开始推送**
7. 观察推送状态和进度条，等待推送完成

## 自动更新

项目已接入 GitHub Release 自动更新，发布新版后可在应用内直接检查更新。

## 说明

- 目录扫描不递归
- 推送前会过滤本地重复和远端已存在账号
- 导入后不会自动绑定分组，需要在 Sub2Api 后台手动处理

## 许可证

本项目采用仓库内 `LICENSE` 所述许可证。
