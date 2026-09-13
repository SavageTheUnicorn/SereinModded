//! Account notification preferences; device sounds and badges are stored separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
	Overview,
	Email,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overview {
	pub streaming: bool,
	pub friend_anniversary: bool,
	pub friends_online: bool,
	pub upcoming_event: bool,
	pub profile_updates: bool,
	/// Discord's enum is retained verbatim so future values are not overwritten.
	pub reactions: u32,
}
impl Default for Overview {
	fn default() -> Self {
		Self {
			streaming: true,
			friend_anniversary: true,
			friends_online: true,
			upcoming_event: true,
			profile_updates: true,
			reactions: 0,
		}
	}
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct Email {
	#[serde(default)]
	pub communication: bool,
	#[serde(default)]
	pub social: bool,
	#[serde(default, rename = "updates_and_announcements")]
	pub announcements: bool,
	#[serde(default)]
	pub tips: bool,
	#[serde(default, rename = "recommendations_and_events")]
	pub recommendations: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
	Streaming(bool),
	FriendAnniversary(bool),
	FriendsOnline(bool),
	UpcomingEvent(bool),
	ProfileUpdates(bool),
	Reactions(u32),
	Communication(bool),
	Social(bool),
	Announcements(bool),
	Tips(bool),
	Recommendations(bool),
	UnsubscribeMarketing,
}
impl Change {
	pub fn section(self) -> Section {
		match self {
			Self::Streaming(_)
			| Self::FriendAnniversary(_)
			| Self::FriendsOnline(_)
			| Self::UpcomingEvent(_)
			| Self::ProfileUpdates(_)
			| Self::Reactions(_) => Section::Overview,
			_ => Section::Email,
		}
	}
	pub fn valid(self) -> bool {
		!matches!(self,Self::Reactions(v) if v>2)
	}
	pub fn apply_overview(self, v: &mut Overview) {
		match self {
			Self::Streaming(b) => v.streaming = b,
			Self::FriendAnniversary(b) => v.friend_anniversary = b,
			Self::FriendsOnline(b) => v.friends_online = b,
			Self::UpcomingEvent(b) => v.upcoming_event = b,
			Self::ProfileUpdates(b) => v.profile_updates = b,
			Self::Reactions(n) => v.reactions = n,
			_ => {}
		}
	}
	pub fn apply_email(self, v: &mut Email) {
		match self {
			Self::Communication(b) => v.communication = b,
			Self::Social(b) => v.social = b,
			Self::Announcements(b) => v.announcements = b,
			Self::Tips(b) => v.tips = b,
			Self::Recommendations(b) => v.recommendations = b,
			Self::UnsubscribeMarketing => {
				v.announcements = false;
				v.tips = false;
				v.recommendations = false;
			}
			_ => {}
		}
	}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snapshot {
	Overview(Overview),
	Email(Email),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocialKind {
	FriendsOnline,
	ProfileUpdates,
	Streaming,
	UpcomingEvent,
	Reaction,
}
impl SocialKind {
	pub fn title(self) -> &'static str {
		match self {
			Self::Streaming => "A friend started streaming",
			Self::FriendsOnline => "A friend came online",
			Self::ProfileUpdates => "A friend updated their profile",
			Self::UpcomingEvent => "Server event",
			Self::Reaction => "New reaction",
		}
	}
}
#[derive(Clone, Debug)]
pub struct SocialNotification {
	pub id: crate::Id,
	pub kind: SocialKind,
	pub body: String,
	pub channel: Option<crate::Id>,
	pub guild: Option<crate::Id>,
	pub other_user: Option<crate::Id>,
}
impl SocialNotification {
	pub fn bytes(&self) -> usize {
		std::mem::size_of::<Self>() + self.body.capacity()
	}
}
