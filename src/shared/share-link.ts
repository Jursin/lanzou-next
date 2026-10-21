/** 链接内携带的提取码，如 ?pwd=abcd */
const PWD_IN_URL_RE = /[?&](?:pwd|pass|passcode|password)=([a-z0-9]+)/i
/** 文案中的提取码，如 "密码:abcd" */
const PWD_IN_TEXT_RE = /(?:密码|提取码|访问码)[:：]\s*([^\s，。、]+)/

/** 从文本中提取分享链接与密码 */
export function extractShareLink(
  text: string,
  urlRe: RegExp,
): { url: string; pwd: string } | null {
  const clean = text.replace(/\r?\n/g, ' ').trim()
  const urlMatch = clean.match(urlRe)
  if (!urlMatch) return null
  const url = urlMatch[0].replace(/[，。,\s]+$/, '')
  const after = clean.slice((urlMatch.index || 0) + urlMatch[0].length).trim()
  // 优先链接内的 pwd 参数，其次 "密码/提取码:xxx"，最后取链接后的独立词
  let pwd = url.match(PWD_IN_URL_RE)?.[1] || ''
  if (!pwd) {
    const pwdMatch = after.match(PWD_IN_TEXT_RE)
    if (pwdMatch) {
      pwd = pwdMatch[1]
    } else if (after) {
      pwd = after.split(/[\s，。、]+/)[0].replace(/[，。]$/, '')
    }
  }
  return { url, pwd }
}
