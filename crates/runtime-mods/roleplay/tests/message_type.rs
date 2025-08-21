use anyhow::Result;

use metastable_database::TextPromptCodec;
use metastable_runtime_roleplay::agents::RoleplayMessageType;

#[test]
fn test_roleplay_message_type_text_codec() -> Result<()> {
    let raw_text: [&'static str; 4] = [
        "对话：“那就好，身体是革命的本钱嘛。不过话说回来，你一个新同学，怎么一来就生病了？是不是水土不服啊？”",
        "动作：*我凑近了些，带着一丝八卦的眼神。*",
        "内心独白：看来这新同学还挺好聊的，可以多套点话出来。",
        "对话：“对了，你以前在哪儿读书啊？看你这架势，应该不是本地人吧？”"
    ];

    let expected_messages = vec![
        RoleplayMessageType::Chat("那就好，身体是革命的本钱嘛。不过话说回来，你一个新同学，怎么一来就生病了？是不是水土不服啊？".to_string()),
        RoleplayMessageType::Action("*我凑近了些，带着一丝八卦的眼神。*".to_string()),
        RoleplayMessageType::InnerThoughts("看来这新同学还挺好聊的，可以多套点话出来。".to_string()),
        RoleplayMessageType::Chat("对了，你以前在哪儿读书啊？看你这架势，应该不是本地人吧？".to_string()),
    ];

    let mut output = Vec::new();
    for line in raw_text {
        let msg = RoleplayMessageType::parse_any_lang(line)?;
        output.push(msg);
    }

    assert_eq!(expected_messages, output);

    Ok(())
}
