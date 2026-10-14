# 3D parity audit (G10, #70)

Each of the 26 headless scenarios of the 2.5D game (`crates/game/tests/scenarios.rs`, deleted in G10; see rc-1 or git
history before G10) and the 3D scenario(s) in `crates/game3d/tests/scenarios.rs` that cover it. 23 had a 3D equivalent
from their own tickets (G1-G9); the 3 marked **G10** were ported for this audit.

| 2.5D scenario | 3D scenario(s) | Note |
|---|---|---|
| `player_spawns_falls_and_comes_to_rest_on_the_floor` | `the_player_lands_on_the_floor_and_stands_still` | |
| `holding_run_moves_the_player_and_releasing_stops_it` | `moves_run_at_full_speed_in_every_direction_and_stop_on_release` | Eight directions, not left/right. |
| `a_dash_covers_its_distance_along_the_floor_and_the_player_stays_grounded` | `a_dash_covers_its_distance_along_the_floor_and_the_player_stays_grounded` | **G10.** |
| `there_is_no_jump` | `there_is_no_jump`; `the_player_cannot_climb_a_wall_or_the_glass` | **G10.** Up runs away from the camera in 3D. |
| `a_punch_lands_on_fodder_in_range_and_not_out_of_range` | `a_punch_lands_on_fodder_in_its_cone_and_not_beside_behind_or_beyond` | Reach becomes a 100° cone (Q49). |
| `attacking_during_a_dash_is_a_drop_kick_that_launches_harder_than_a_punch` | `a_dash_kick_launches_harder_than_a_punch` | |
| `fodder_enter_through_a_door_only_after_its_warning` | `fodder_enter_through_a_door_only_after_its_warning`; `doors_deliver_thugs_and_ninjas_when_the_box_calls_for_them` | **G10.** Counts the warning before the first entry only. |
| `a_defeated_enemy_leaves_its_capped_number_of_solid_parts` | `a_defeated_fodder_bursts_into_its_pieces_which_tumble_then_freeze_on_the_floor`; `a_mass_burst_keeps_at_most_the_moving_cap_in_motion_and_all_of_it_freezes`; `past_the_frozen_cap_the_oldest_frozen_pieces_go` | Six parts per enemy become the figure's pieces; caps are 300 moving and 1,500 frozen (Q52). |
| `debris_freezes_once_settled_per_the_spike_rule` | `a_defeated_fodder_bursts_into_its_pieces_which_tumble_then_freeze_on_the_floor` | |
| `the_player_walks_up_a_ramp_of_debris` | `the_player_steps_up_onto_a_piece_lying_on_the_floor_but_not_onto_a_waist_high_block`; `the_player_climbs_a_pile_of_debris_and_escapes_to_box_2`; `a_heap_of_bursts_built_and_climbed_at_30_fps_stays_put_under_the_player` | Reshaped: step onto pieces, climb a settled pile (Q59). |
| `building_the_pile_under_the_player_to_the_rim_moves_on_to_a_taller_box` | `the_player_climbs_a_pile_of_debris_and_escapes_to_box_2`; `the_demo_stairs_run_climbs_out_of_box_after_box` | Escape when the pile under the footprint reaches the rim (Q51). |
| `running_out_of_time_costs_health_but_a_healthy_player_carries_on` | `running_out_of_time_costs_health_and_on_the_last_of_it_ends_the_run` | |
| `death_shows_the_game_over_screen_and_enter_starts_a_fresh_run` | `running_out_of_time_costs_health_and_on_the_last_of_it_ends_the_run`; `a_defeat_ends_the_run_and_the_next_starts_at_box_1_in_an_empty_box` | |
| `a_grabbed_frozen_part_thaws_and_once_thrown_lands_as_debris` | `frozen_debris_thaws_when_grabbed_and_once_thrown_freezes_again` | |
| `a_thrown_enemy_damages_the_enemies_it_hits` | `a_spin_throw_through_a_line_of_fodder_hurts_each_once_and_the_thrown_one_once` | Reshaped: a spin-throw that passes through the line (Q62). |
| `escape_pauses_the_clock_and_resumes_it` | `esc_pauses_everything_and_esc_again_resumes_it` | |
| `kills_score_points_and_a_new_run_starts_from_zero` | `a_run_starts_from_the_overlay_scores_a_kill_and_ends_on_the_game_over_screen_with_a_new_best_kept` | |
| `a_punch_lands_even_when_a_slow_frame_skips_its_active_window` | `a_blow_lands_even_on_slow_frames` | |
| `punches_hurt_a_thug_but_dont_interrupt_its_wind_up` | `punches_hurt_a_thug_but_dont_interrupt_its_wind_up` | |
| `a_kick_interrupts_a_thugs_wind_up` | `a_kick_interrupts_a_thugs_wind_up` | |
| `doors_deliver_thugs_when_the_box_calls_for_them` | `doors_deliver_thugs_and_ninjas_when_the_box_calls_for_them` | |
| `a_ninja_strikes_from_behind_and_backs_off` | `a_ninja_strikes_from_behind_and_backs_off` | |
| `a_ninja_runs_through_rather_than_standing_in_front` | `a_ninja_faced_circles_round_to_flank_the_player_rather_than_running_through` | Reshaped: a ninja circles round to flank (G5). |
| `a_blast_knocks_debris_loose_without_deleting_any` | `a_blast_thaws_and_flings_frozen_debris_in_range_within_the_moving_cap` | Within the moving cap. |
| `a_defeated_bomb_head_drops_a_lit_bomb_instead_of_its_head` | `a_defeated_bomb_head_drops_its_head_as_a_lit_bomb_which_blows_without_deleting_debris`; `thugs_and_ninjas_burst_into_their_own_pieces_and_a_bomb_head_drops_its_head` | |
| `a_lit_bomb_can_be_grabbed_and_thrown` | `a_thrown_bomb_stops_in_the_group_it_hits_and_its_blast_hurts_them_all` | Reshaped: the bomb stops at the first enemy it hits. |
