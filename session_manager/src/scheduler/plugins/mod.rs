/*
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use stdng::collections;
use stdng::{lock_ptr, new_ptr, MutexPtr};

use crate::model::{ExecutorInfoPtr, NodeInfo, NodeInfoPtr, SessionInfo, SessionInfoPtr, SnapShot};
use crate::scheduler::plugins::fairshare::FairShare;
use crate::scheduler::Context;

use common::FlameError;

mod fairshare;

pub type PluginPtr = Box<dyn Plugin>;
pub type PluginManagerPtr = Arc<PluginManager>;

pub trait Plugin: Send + Sync + 'static {
    // Installation of plugin
    fn setup(&mut self, ss: &SnapShot) -> Result<(), FlameError>;

    // Order Fn
    fn ssn_order_fn(&self, s1: &SessionInfo, s2: &SessionInfo) -> Option<Ordering> {
        None
    }
    fn node_order_fn(&self, s1: &NodeInfo, s2: &NodeInfo) -> Option<Ordering> {
        None
    }

    // Filter Fn
    fn is_underused(&self, ssn: &SessionInfoPtr) -> Option<bool> {
        None
    }

    fn is_preemptible(&self, ssn: &SessionInfoPtr) -> Option<bool> {
        None
    }

    fn is_available(&self, exec: &ExecutorInfoPtr, ssn: &SessionInfoPtr) -> Option<bool> {
        None
    }

    fn is_allocatable(&self, node: &NodeInfoPtr, ssn: &SessionInfoPtr) -> Option<bool> {
        None
    }

    fn is_reclaimable(&self, exec: &ExecutorInfoPtr) -> Option<bool> {
        None
    }

    // Events callbacks
    fn on_create_executor(&mut self, node: NodeInfoPtr, ssn: SessionInfoPtr) {}

    fn on_session_bind(&mut self, ssn: SessionInfoPtr) {}

    fn on_session_unbind(&mut self, ssn: SessionInfoPtr) {}
}

pub struct PluginManager {
    pub plugins: MutexPtr<HashMap<String, PluginPtr>>,
}

impl PluginManager {
    pub fn setup(ss: &SnapShot) -> Result<PluginManagerPtr, FlameError> {
        let mut plugins = HashMap::from([("fairshare".to_string(), FairShare::new_ptr())]);

        for plugin in plugins.values_mut() {
            plugin.setup(ss)?;
        }

        Ok(Arc::new(PluginManager {
            plugins: new_ptr(plugins),
        }))
    }

    pub fn is_underused(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        let plugins = lock_ptr!(self.plugins)?;

        Ok(plugins
            .values()
            .any(|plugin| plugin.is_underused(ssn).unwrap_or(false)))
    }

    pub fn is_preemptible(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        let plugins = lock_ptr!(self.plugins)?;

        Ok(plugins
            .values()
            .all(|plugin| plugin.is_preemptible(ssn).unwrap_or(false)))
    }

    pub fn is_available(
        &self,
        exec: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<bool, FlameError> {
        let plugins = lock_ptr!(self.plugins)?;

        Ok(plugins
            .values()
            .all(|plugin| plugin.is_available(exec, ssn).unwrap_or(true)))
    }

    pub fn is_allocatable(
        &self,
        node: &NodeInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<bool, FlameError> {
        let plugins = lock_ptr!(self.plugins)?;

        Ok(plugins
            .values()
            .all(|plugin| plugin.is_allocatable(node, ssn).unwrap_or(true)))
    }

    pub fn is_reclaimable(&self, exec: &ExecutorInfoPtr) -> Result<bool, FlameError> {
        let plugins = lock_ptr!(self.plugins)?;

        Ok(plugins
            .values()
            .all(|plugin| plugin.is_reclaimable(exec).unwrap_or(true)))
    }

    pub fn on_create_executor(
        &self,
        node: NodeInfoPtr,
        ssn: SessionInfoPtr,
    ) -> Result<(), FlameError> {
        let mut plugins = lock_ptr!(self.plugins)?;

        for plugin in plugins.values_mut() {
            plugin.on_create_executor(node.clone(), ssn.clone());
        }

        Ok(())
    }

    pub fn on_session_bind(&self, ssn: SessionInfoPtr) -> Result<(), FlameError> {
        let mut plugins = lock_ptr!(self.plugins)?;

        for plugin in plugins.values_mut() {
            plugin.on_session_bind(ssn.clone());
        }

        Ok(())
    }

    pub fn on_session_unbind(&self, ssn: SessionInfoPtr) -> Result<(), FlameError> {
        let mut plugins = lock_ptr!(self.plugins)?;

        for plugin in plugins.values_mut() {
            plugin.on_session_unbind(ssn.clone());
        }
        Ok(())
    }

    pub fn ssn_order_fn(&self, t1: &SessionInfoPtr, t2: &SessionInfoPtr) -> Ordering {
        if let Ok(plugins) = lock_ptr!(self.plugins) {
            for plugin in plugins.values() {
                if let Some(order) = plugin.ssn_order_fn(t1, t2) {
                    if order != Ordering::Equal {
                        return order;
                    }
                }
            }
        }

        Ordering::Equal
    }

    pub fn node_order_fn(&self, t1: &NodeInfoPtr, t2: &NodeInfoPtr) -> Ordering {
        if let Ok(plugins) = lock_ptr!(self.plugins) {
            for plugin in plugins.values() {
                if let Some(order) = plugin.node_order_fn(t1, t2) {
                    if order != Ordering::Equal {
                        return order;
                    }
                }
            }
        }
        Ordering::Equal
    }
}

pub fn node_order_fn(ctx: &Context) -> impl collections::Cmp<NodeInfoPtr> {
    NodeOrderFn {
        plugin_mgr: ctx.plugins.clone(),
    }
}

struct NodeOrderFn {
    plugin_mgr: PluginManagerPtr,
}

impl collections::Cmp<NodeInfoPtr> for NodeOrderFn {
    fn cmp(&self, t1: &NodeInfoPtr, t2: &NodeInfoPtr) -> Ordering {
        self.plugin_mgr.node_order_fn(t1, t2)
    }
}

pub fn ssn_order_fn(ctx: &Context) -> impl collections::Cmp<SessionInfoPtr> {
    SsnOrderFn {
        plugin_mgr: ctx.plugins.clone(),
    }
}

struct SsnOrderFn {
    plugin_mgr: PluginManagerPtr,
}

impl collections::Cmp<SessionInfoPtr> for SsnOrderFn {
    fn cmp(&self, t1: &SessionInfoPtr, t2: &SessionInfoPtr) -> Ordering {
        self.plugin_mgr.ssn_order_fn(t1, t2)
    }
}
