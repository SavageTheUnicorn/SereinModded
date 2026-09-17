//! One bounded, session-only composer member lookup.
use crate::{Command, State, auth::Failure};
use model::{Id, Member};

pub const LIMIT: usize = 100;
pub const MAX_BYTES: usize = 128 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
	pub guild: Id,
	pub channel: Id,
	pub query: String,
	pub nonce: u64,
	pub slot: usize,
}
impl Request {
	pub fn valid(&self) -> bool {
		self.guild.0 != 0
			&& self.channel.0 != 0
			&& self.slot == 0
			&& !self.query.is_empty()
			&& self.query.len() <= 256
			&& self.query.chars().count() <= 64
			&& !self.query.chars().any(char::is_control)
	}
}
#[derive(Default)]
pub struct View {
	pub request: Option<Request>,
	pub rows: Vec<Member>,
	pub finished: bool,
	pub error: Option<&'static str>,
}
impl State {
	pub fn search_members(&mut self, channel: Id, query: &str, slot: usize) -> Option<Command> {
		let guild = self.channel(channel)?.guild?;
		if slot != 0
			|| !self.gateway_connected
			|| !self.can_view(channel)
			|| self.selected != Some(channel)
		{
			return None;
		}
		self.member_search_nonce = self.member_search_nonce.wrapping_add(1);
		let request = Request {
			guild,
			channel,
			query: query.to_owned(),
			nonce: self.member_search_nonce,
			slot,
		};
		if !request.valid() {
			return None;
		}
		self.member_search[slot] = View {
			request: Some(request.clone()),
			..Default::default()
		};
		Some(Command::MemberSearch(request))
	}
	pub(crate) fn searched_members(
		&mut self,
		request: Request,
		result: Result<Vec<Member>, Failure>,
	) {
		if !request.valid()
			|| self.selected != Some(request.channel)
			|| !self.gateway_connected
			|| !self.can_view(request.channel)
			|| self.channel(request.channel).and_then(|c| c.guild) != Some(request.guild)
			|| self.member_search[request.slot].request.as_ref() != Some(&request)
		{
			return;
		}
		let view = &mut self.member_search[request.slot];
		view.finished = true;
		match result {
			Ok(rows)
				if rows.len() <= LIMIT
					&& rows.iter().all(Member::valid)
					&& rows.iter().map(Member::bytes).sum::<usize>() <= MAX_BYTES =>
			{
				view.rows = rows
			}
			Ok(_) => view.error = Some("Member search exceeded its safety limit"),
			Err(error) => view.error = Some(error.label()),
		}
	}
}
