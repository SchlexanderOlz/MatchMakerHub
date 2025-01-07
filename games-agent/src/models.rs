use itertools::Itertools;

use gn_matchmaking_state_types::ActiveMatchDB;


pub struct MatchResultMaker(gn_communicator::models::MatchResult, ActiveMatchDB);

impl From<(gn_communicator::models::MatchResult, ActiveMatchDB)> for MatchResultMaker {
    fn from(x: (gn_communicator::models::MatchResult, ActiveMatchDB)) -> Self {
        MatchResultMaker(x.0, x.1)
    }
}

impl Into<gn_ranking_client_rs::models::create::Match> for MatchResultMaker {
    fn into(self) -> gn_ranking_client_rs::models::create::Match {
        let active_match = self.1;
        let result = self.0;

        gn_ranking_client_rs::models::create::Match {
        game_name: active_match.game.clone(),
        game_mode: active_match.mode.clone(),
        player_match_list: result
            .ranking
            .performances
            .into_iter()
            .map(
                |(player_id, performances)| gn_ranking_client_rs::models::create::PlayerMatch {
                    player_id: player_id.clone(),
                    player_performances: performances
                        .into_iter()
                        .counts()
                        .into_iter()
                        .map(
                            |x| gn_ranking_client_rs::models::create::PlayerPerformance {
                                name: x.0,
                                count: x.1 as i32,
                            },
                        )
                        .chain(std::iter::once(gn_ranking_client_rs::models::create::PlayerPerformance {
                            name: "point".to_owned(),
                            count: result.winners.get(&player_id).unwrap_or(result.losers.get(&player_id).unwrap()).clone() as i32,
                        }))
                        .collect(),
                },
            )
            .collect(),
    }
    }
}
