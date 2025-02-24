use super::EntTestState;
use anyhow::Result;

impl EntTestState {
    pub fn assert_object_exists(&self, index: usize) -> Result<()> {
        match self.get_object(index) {
            Some(_) => Ok(()),
            None => Err(anyhow::anyhow!("Object at index {} does not exist", index)),
        }
    }

    pub fn assert_edge_exists(&self, from_index: usize, to_index: usize) -> Result<()> {
        let edge_exists = self.edges.iter().any(|edge| {
            let from_obj = self.get_object(from_index);
            let to_obj = self.get_object(to_index);

            matches!((from_obj, to_obj), (Some(from), Some(to)) if
                edge.edge.from_id == from.id &&
                edge.edge.to_id == to.id
            )
        });

        if edge_exists {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Edge from {} to {} does not exist",
                from_index,
                to_index
            ))
        }
    }

    pub fn assert_user_authenticated(&self, user_index: usize) -> Result<()> {
        match self.get_user_token(user_index) {
            Some(_) => Ok(()),
            None => Err(anyhow::anyhow!(
                "User at index {} is not authenticated",
                user_index
            )),
        }
    }
}
