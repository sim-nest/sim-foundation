//! Deterministic cookbook builders for surface-card recipes.

use sim_kernel::Symbol;

use crate::{ExternalNamePolicy, external_name};

/// Report produced by the external-name cookbook recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalNameDemo {
    /// Kernel symbol being projected.
    pub symbol: String,
    /// OpenAI tool-compatible external name.
    pub openai_tool: String,
    /// MCP tool-compatible external name.
    pub mcp_tool: String,
    /// Human-readable external name.
    pub human_readable: String,
}

/// Build the modeled external-name projection report used by the cookbook.
pub fn external_name_demo() -> ExternalNameDemo {
    let symbol = Symbol::qualified("skill", "do.thing");
    ExternalNameDemo {
        symbol: symbol.as_qualified_str(),
        openai_tool: external_name(&symbol, ExternalNamePolicy::OpenAiTool),
        mcp_tool: external_name(&symbol, ExternalNamePolicy::McpTool),
        human_readable: external_name(&symbol, ExternalNamePolicy::HumanReadable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_name_demo_applies_surface_policies() {
        let demo = external_name_demo();

        assert_eq!(demo.symbol, "skill/do.thing");
        assert_eq!(demo.openai_tool, "skill_do_thing");
        assert_eq!(demo.mcp_tool, "skill_do_thing");
        assert_eq!(demo.human_readable, "skill/do.thing");
    }
}
