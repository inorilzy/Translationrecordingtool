use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let app_key = "54760493bd50abce";
    let app_secret = "你的密钥"; // 需要替换成真实密钥
    let text = "hello";

    let salt = "12345678";
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    // 计算 input
    let input = if text.len() > 20 {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let start: String = chars.iter().take(10).collect();
        let end: String = chars.iter().skip(len - 10).collect();
        format!("{}{}{}", start, len, end)
    } else {
        text.to_string()
    };

    // 签名计算
    let sign_str = format!("{}{}{}{}{}", app_key, input, salt, timestamp, app_secret);
    let sign = format!("{:x}", md5::compute(&sign_str));

    println!("文本: {}", text);
    println!("Input: {}", input);
    println!("Salt: {}", salt);
    println!("Timestamp: {}", timestamp);
    println!("签名字符串: {}", sign_str);
    println!("签名: {}", sign);
    println!("\n请求 URL:");
    println!("https://openapi.youdao.com/api?q={}&from=auto&to=zh-CHS&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
        text, app_key, salt, sign, timestamp);
}
