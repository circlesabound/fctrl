
// fn try_parse_console_out_message(line: &str) -> Option<ConsoleOutMessage> {
//     lazy_static! {
//         static ref CHAT_RE: Regex =
//             Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] ([^:]+): (.+)$").unwrap();
//         static ref JOIN_RE: Regex = Regex::new(
//             r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+): joined the game$"
//         )
//         .unwrap();
//         static ref LEAVE_RE: Regex =
//             Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$")
//                 .unwrap();
//     }

//     if let Some(chat_captures) = CHAT_RE.captures(line) {
//         let timestamp = chat_captures.get(1).unwrap().as_str().to_string();
//         let user = chat_captures.get(2).unwrap().as_str().to_string();
//         let msg = chat_captures.get(3).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Chat {
//             timestamp,
//             user,
//             msg,
//         })
//     } else if let Some(join_captures) = JOIN_RE.captures(line) {
//         let timestamp = join_captures.get(1).unwrap().as_str().to_string();
//         let user = join_captures.get(2).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Join { timestamp, user })
//     } else if let Some(leave_captures) = LEAVE_RE.captures(line) {
//         let timestamp = leave_captures.get(1).unwrap().as_str().to_string();
//         let user = leave_captures.get(2).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Leave { timestamp, user })
//     } else {
//         None
//     }
// }