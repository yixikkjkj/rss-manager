const labels: Record<string, string> = {
  success: '抓取成功',
  not_modified: '内容未更新',
  network_error: '网络请求失败',
  auth_required: '需要身份认证',
  access_denied: '访问被拒绝',
  challenge_required: '疑似人机验证',
  rate_limited: '请求频率受限',
  unexpected_content: '响应内容异常',
  parse_error: 'Feed 格式错误',
  http_error: 'HTTP 错误',
};
export const fetchResultLabel = (result: string) => labels[result] ?? result;
