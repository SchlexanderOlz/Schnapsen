use std::sync::{Arc, Mutex};

pub struct TokioTaskScheduler{
    pub handles: Vec<tokio::task::JoinHandle<()>>,
}


impl TokioTaskScheduler{
    pub fn new() -> Self{
        TokioTaskScheduler{
            handles: Vec::new(),
        }
    }

    pub fn add_task(&mut self, task: tokio::task::JoinHandle<()>) {
        self.handles.push(task);
    }

    pub fn add_tasks(&mut self, tasks: Vec<tokio::task::JoinHandle<()>>) {
        self.handles.extend(tasks);
    }

    pub async fn wait_for_all(&mut self){
        for handle in self.handles.iter_mut(){
            handle.await.unwrap();
        }
    }
}