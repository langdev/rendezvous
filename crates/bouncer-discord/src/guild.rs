use std::collections::HashMap;

use serenity::model::prelude::*;

pub type GuildMap = HashMap<GuildId, GuildData>;

pub struct GuildData {
    pub members: HashMap<UserId, UserData>,
}

pub struct UserData {
    pub id: UserId,
    pub name: String,
}

impl From<Guild> for GuildData {
    fn from(g: Guild) -> Self {
        Self {
            members: g.members.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<Member> for UserData {
    fn from(m: Member) -> Self {
        let Member { nick, user, .. } = m;
        let u = user.read();
        Self {
            id: u.id,
            name: nick.unwrap_or_else(|| u.name.clone()),
        }
    }
}

impl From<GuildMemberUpdateEvent> for UserData {
    fn from(e: GuildMemberUpdateEvent) -> Self {
        let GuildMemberUpdateEvent { nick, user, .. } = e;
        Self {
            id: user.id,
            name: nick.unwrap_or_else(|| user.name),
        }
    }
}

fn author_nickname<'a>(g: &'a GuildMap, message: &Message) -> Option<&'a str> {
    let guild = g.get(&message.guild_id?)?;
    Some(guild.members.get(&message.author.id)?.name.as_str())
}

pub fn author_name<'a>(g: &'a GuildMap, message: &'a Message) -> &'a str {
    author_nickname(g, message).unwrap_or_else(|| &message.author.name[..])
}

#[cfg(test)]
mod test {
    use serenity::utils::CustomMessage;

    use super::*;

    #[test]
    fn author_nickname_empty_guild_map() {
        let guild_map = GuildMap::new();
        let mut message = CustomMessage::new();
        message.guild_id(GuildId(100200));
        let message = message.build();
        assert!(author_nickname(&guild_map, &message).is_none());
    }
}
