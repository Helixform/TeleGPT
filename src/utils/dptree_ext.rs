use teloxide::prelude::*;
use teloxide::types::Me;

pub trait HandlerExt {
    fn post_chain(self, next: Self) -> Self;
}

impl<'a, Input, Output, Descr> HandlerExt for Handler<'a, Input, Output, Descr>
where
    Input: Clone + Send + 'a,
    Output: 'a,
    Descr: dptree::HandlerDescription,
{
    fn post_chain(self, next: Self) -> Self {
        dptree::from_fn(move |input: Input, cont| {
            let handler = self.clone();
            let next = next.clone();
            let input_clone = input.clone();
            async {
                let _ = handler
                    .execute(
                        input_clone,
                        |event| async move { ControlFlow::Continue(event) },
                    )
                    .await;
                next.execute(input, cont).await
            }
        })
    }
}

fn extract_command_args<'i>(input: &'i str, cmd: &str, username: &str) -> Option<&'i str> {
    let pat = format!("/{}", cmd);
    input.strip_prefix(&pat).and_then(|rest| {
        if rest.is_empty() {
            return Some(rest);
        }

        // When sending commands in a group, a mention suffix may be attached to
        // the text. For example: "/reset@xxxx_bot".
        let mention_part = format!("@{}", username);
        let args = rest.strip_prefix(&mention_part).unwrap_or(rest);

        if args.is_empty() {
            return Some(args);
        }

        return args.strip_prefix(' ');
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandArgs(pub String);

pub fn command_filter(cmd: &'static str) -> impl Fn(Message, Me) -> Option<CommandArgs> {
    move |msg: Message, me: Me| {
        let text = msg.text()?;
        extract_command_args(text, cmd, me.username()).map(|a| CommandArgs(a.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::extract_command_args;

    #[test]
    fn test_extract_command_args() {
        let username = "mybot";
        assert!(matches!(
            extract_command_args("/test", "test", username),
            Some("")
        ));
        assert!(matches!(
            extract_command_args("/test1", "test", username),
            None
        ));
        assert!(matches!(
            extract_command_args("/test@otherbot", "test", username),
            None
        ));
        assert!(matches!(
            extract_command_args("/test@mybot", "test", username),
            Some("")
        ));
        assert!(matches!(
            extract_command_args("/test@mybot arg1 arg2", "test", username),
            Some("arg1 arg2")
        ));
        assert!(matches!(
            extract_command_args("/test@mybotarg", "test", username),
            None
        ));
        assert!(matches!(
            extract_command_args("/test arg1 arg2", "test", username),
            Some("arg1 arg2")
        ));
    }
}
