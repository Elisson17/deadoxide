//! zombie.ai.states.PathFindState
use crate::zombie::characters::iso_zombie::IsoZombie;
use crate::zombie::pathfind::path_find_behavior2::BehaviorResult;

impl IsoZombie {
    /// zombie.ai.states.PathFindState.enter: `bPathfind` true, `bMoving` false (the `extraUpdate` of the network AI only
    /// queues the zombie for the network simulator), the tick counter reset.
    pub(crate) fn path_find_enter(&mut self) -> Result<(), String> {
        self.sp_set_bool("bPathfind", true)?;
        self.sp_set_bool("bMoving", false)?;
        self.sp.path_find_tick_count = 0;
        Ok(())
    }

    /// zombie.ai.states.PathFindState.execute: `PathFindBehavior2.update()`. A request that never completes (measured:
    /// the polygonal map's thread does not answer headless) leaves the zombie here counting ticks (the tick-2 audio
    /// parameter `ParameterZombieState.Idle` is not modelled). `Failed` and `Succeeded` need a finished search: `Err`.
    pub(crate) fn path_find_execute(&mut self) -> Result<(), String> {
        match self.path.update()? {
            BehaviorResult::Failed => Err(
                "PathFindState.execute: a failed path search is not ported (the request never completes headless)"
                    .into(),
            ),
            BehaviorResult::Succeeded => Err(
                "PathFindState.execute: a found path is not ported (the request never completes headless)"
                    .into(),
            ),
            BehaviorResult::Working => {
                self.sp.path_find_tick_count += 1;
                Ok(())
            }
        }
    }

    /// zombie.ai.states.PathFindState.exit: `allowRepathDelay` 0, the path variables cleared, the request cancelled,
    /// the finder `notrunning`, no path.
    pub(crate) fn path_find_exit(&mut self) -> Result<(), String> {
        self.allow_repath_delay = 0.0;
        self.sp_set_bool("bPathfind", false)?;
        self.sp_set_bool("bMoving", false)?;
        self.sp_set_bool("ShouldBeCrawling", false)?;
        self.path.cancel_request();
        Ok(())
    }
}
