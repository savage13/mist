use super::Comparison as Comp;
use super::MistInstant;
use super::Run;
use std::cell::RefCell;
use std::rc::Rc;

pub struct RunState {
    run: Rc<RefCell<Run>>,
    timer: MistInstant,
    timer_state: TimerState,
    run_status: SplitStatus,
    comparison: Comp,
    run_times: Vec<u128>,
    run_diffs: Vec<i128>,
    before_pause: u128,
    before_pause_split: u128,
    split: u128,
    start: u128,
    current_split: usize,
    needs_save: bool,
}

#[derive(PartialEq)]
pub enum TimerState {
    Running,
    NotRunning,
    Paused,
    Offset,
    Finished,
}

pub enum StateChangeRequest {
    None,
    Pause,
    Split,
    Reset,
    Comparison(bool),
}

// commented items will be used for plugins later
pub enum StateChange {
    None,
    EnterOffset, /*{amt: u128}*/
    ExitOffset,
    EnterSplit {
        idx: usize, /*name: String, pb: u128, gold: u128 */
    },
    ExitSplit {
        idx: usize,
        /*name: String,*/ status: SplitStatus,
        time: u128,
    },
    Pause,
    Unpause {
        status: SplitStatus,
    },
    Finish,
    Reset {
        offset: Option<u128>,
    },
    ComparisonChanged {
        comp: Comp
    },
}

pub struct RunUpdate {
    pub change: Vec<StateChange>,
    pub time: u128,
    pub offset: bool,
    pub status: SplitStatus,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SplitStatus {
    None,
    Ahead,
    Gaining,
    Gold,
    Behind,
    Losing,
}

impl RunState {
    pub fn new(run: Rc<RefCell<Run>>) -> Self {
        Self {
            run,
            timer: MistInstant::now(),
            timer_state: TimerState::NotRunning,
            comparison: Comp::PersonalBest,
            run_status: SplitStatus::None,
            run_times: vec![],
            run_diffs: vec![],
            before_pause: 0,
            before_pause_split: 0,
            split: 0,
            start: 0,
            current_split: 0,
            needs_save: false,
        }
    }
    pub fn update(&mut self, rq: StateChangeRequest) -> RunUpdate {
        // TODO logic for checking offset stuff
        let elapsed = self.timer.elapsed().as_millis();
        self.run_status = self.calc_status(elapsed);
        RunUpdate {
            change: self.handle_scrq(rq, elapsed),
            time: (elapsed - self.start) + self.before_pause,
            offset: false, // TODO
            status: self.run_status,
        }
    }
    fn calc_status(&self, elapsed: u128) -> SplitStatus {
        if self.comparison == Comp::None || self.timer_state != TimerState::Running {
            return SplitStatus::None;
        }
        let time = (elapsed - self.start) + self.before_pause;
        let run = self.run.borrow();
        if run.pb_times().len() == 0 {
            if time < run.pb() {
                SplitStatus::Ahead
            } else {
                SplitStatus::Behind
            }
        } else {
            let buffer = if self.current_split != 0 {
                self.run_diffs[self.current_split - 1]
            } else {
                0
            };
            let allowed = (match self.comparison {
                Comp::PersonalBest => run.pb_times()[self.current_split],
                Comp::Golds => run.gold_times()[self.current_split],
                Comp::Average => {
                    let sum = run.sum_times()[self.current_split];
                    sum.1 / {
                        if sum.0 == 0 {
                            1
                        } else {
                            sum.0
                        }
                    }
                }
                _ => unreachable!(),
            }) as i128
                - buffer;
            let time = ((elapsed - self.split) + self.before_pause_split) as i128;
            // if the last split was ahead of comparison split
            if buffer < 0 {
                // if the runner has spent more time than allowed they have to be behind
                if time > allowed {
                    SplitStatus::Behind
                // if they have spent less than the time it would take to become behind but more time than they took in the pb,
                // then they are losing time but still ahead. default color for this is lightish green like LiveSplit
                } else if time < allowed && time > allowed + buffer {
                    SplitStatus::Losing
                // if neither of those are true the runner must be ahead
                } else {
                    SplitStatus::Ahead
                }
            // if last split was behind comparison split
            } else {
                // if the runner has gone over the amount of time they should take but are still on better pace than
                // last split then they are making up time. a sort of light red color like livesplit
                if time > allowed && time < allowed + buffer {
                    SplitStatus::Gaining
                // if they are behind both the allowed time and their current pace they must be behind
                } else if time > allowed && time > allowed + buffer {
                    SplitStatus::Behind
                // even if the last split was behind, often during part of the split the runner could finish it and come out ahead
                } else {
                    SplitStatus::Ahead
                }
            }
        }
    }
    fn handle_scrq(&mut self, rq: StateChangeRequest, elapsed: u128) -> Vec<StateChange> {
        use StateChangeRequest::*;
        match rq {
            Pause
                if self.timer_state == TimerState::Running
                    || self.timer_state == TimerState::Offset =>
            {
                self.timer_state = TimerState::Paused;
                self.before_pause = (elapsed - self.start) + self.before_pause;
                self.before_pause_split = (elapsed - self.split) + self.before_pause_split;
                return vec![StateChange::Pause];
            }
            Pause if self.timer_state == TimerState::Paused => {
                self.timer_state = TimerState::Running;
                self.start = elapsed;
                self.split = elapsed;
                return vec![StateChange::Unpause {
                    status: self.run_status,
                }];
            }
            Split if self.timer_state == TimerState::Running => {
                // TODO run updates/save file updates etc
                let time = (elapsed - self.split) + self.before_pause_split;
                self.run_times.push(time);
                self.run_diffs
                    .push(if self.comparison == Comp::PersonalBest {
                        self.run.borrow().pb_times()[self.current_split] as i128 - time as i128
                    } else if self.comparison == Comp::Golds {
                        self.run.borrow().gold_times()[self.current_split] as i128 - time as i128
                    } else if self.comparison == Comp::Average {
                        let sum = self.run.borrow().sum_times()[self.current_split];
                        (sum.1 / {
                            if sum.0 == 0 {
                                1
                            } else {
                                sum.0
                            }
                        }) as i128
                            - time as i128
                    } else {
                        0
                    });
                if time < self.run.borrow().gold_times()[self.current_split] {
                    self.run
                        .borrow_mut()
                        .set_gold_time(time, self.current_split);
                    self.needs_save = true;
                }
                if self.current_split == self.run.borrow().pb_times().len() - 1 {
                    self.timer_state = TimerState::Finished;
                    if time < self.run.borrow().pb() {
                        self.needs_save = true;
                        self.run.borrow_mut().set_pb(time);
                        self.run.borrow_mut().set_pb_times(&self.run_times);
                    }
                    return vec![
                        StateChange::ExitSplit {
                            idx: self.current_split,
                            time: self.run_times[self.current_split],
                            status: self.run_status,
                        },
                        StateChange::Finish,
                    ];
                } else {
                    self.current_split += 1;
                    return vec![
                        StateChange::ExitSplit {
                            idx: self.current_split - 1,
                            time,
                            status: self.run_status,
                        },
                        StateChange::EnterSplit {
                            idx: self.current_split,
                        },
                    ];
                }
            }
            Split if self.timer_state == TimerState::NotRunning => {
                if self.run.borrow().offset().is_some() {
                    self.timer_state = TimerState::Offset;
                    return vec![StateChange::EnterOffset];
                } else {
                    self.timer_state = TimerState::Running;
                    return vec![StateChange::EnterSplit { idx: 0 }];
                }
            }
            Reset => {
                self.before_pause = 0;
                self.before_pause_split = 0;
                self.split = 0;
                self.start = 0;
                self.run_diffs = vec![];
                self.run_times = vec![];
                self.current_split = 0;
                self.timer_state = TimerState::NotRunning;
                return vec![StateChange::Reset {
                    offset: self.run.borrow().offset(),
                }];
            }
            Comparison(n) => {
                if n {
                    self.comparison.next();
                } else {
                    self.comparison.prev();
                }
                return vec![StateChange::ComparisonChanged {comp: self.comparison}];
            }
            _ => {}
        }
        vec![StateChange::None]
    }
}
