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

        let winners = result.winners.into_iter().sorted_by(|a, b| b.1.cmp(&a.1));
        let losers = result.losers.into_iter().sorted_by(|a, b| b.1.cmp(&a.1));

        gn_ranking_client_rs::models::create::Match {
            game_name: active_match.game.clone(),
            game_mode: active_match.mode.clone(),
            player_match_list: winners
                .chain(losers)
                .map(
                    |(player_id, _)| gn_ranking_client_rs::models::create::PlayerMatch {
                        player_performances: result
                            .ranking
                            .performances
                            .get(&player_id)
                            .unwrap_or(&vec![])
                            .into_iter()
                            .counts()
                            .into_iter()
                            .map(
                                |x| gn_ranking_client_rs::models::create::PlayerPerformance {
                                    name: x.0.clone(),
                                    count: x.1 as i32,
                                },
                            )
                            .collect(),
                        player_id,
                    },
                )
                .collect(),
        }
    }
}
