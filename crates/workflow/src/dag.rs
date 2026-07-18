use ade_core::plan::Phase;

pub struct DagBuilder;

impl Default for DagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DagBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, phases: Vec<Phase>) -> Result<Vec<Phase>, String> {
        let sorted = phases;
        // TODO: topological sort by depends_on
        Ok(sorted)
    }
}
