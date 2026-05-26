# MCD MCP Python Quickstart

This guide is for someone who has a Python model or prompt harness and wants it
to read `.mcd` documents through MCP.

MCP is the bridge between your model and external tools. The MCD MCP server is a
local tool server; your Python app starts it, lists its tools, and calls those
tools when the model asks for document search, table queries, metadata, or
rendering.

## 1. Install the MCD MCP Server

On Windows, use the prebuilt binary. This avoids needing Rust, Visual Studio, or
`link.exe`.

```bat
mkdir "%TEMP%\mcd-mcp" 2>NUL
curl -L -o "%TEMP%\mcd-mcp-windows-x64.zip" https://github.com/NikitaSukhikh/mcd/releases/download/v0.1.0-alpha.2/mcd-mcp-windows-x64.zip
tar -xf "%TEMP%\mcd-mcp-windows-x64.zip" -C "%TEMP%\mcd-mcp"
mkdir "%USERPROFILE%\.cargo\bin" 2>NUL
copy /Y "%TEMP%\mcd-mcp\mcd-mcp.exe" "%USERPROFILE%\.cargo\bin\mcd-mcp.exe"
```

Verify:

```bat
"%USERPROFILE%\.cargo\bin\mcd-mcp.exe" --help
```

If `%USERPROFILE%\.cargo\bin` is on `PATH`, this should also work:

```bat
mcd-mcp --help
```

For macOS or Linux users who already have Rust installed:

```bash
cargo install mcd-mcp --version 0.1.0-alpha.2
mcd-mcp --help
```

## 2. Install the Python MCP Client SDK

In your Python project:

```bat
cd path\to\your\project
.venv\Scripts\activate
python -m pip install mcp
```

On macOS or Linux:

```bash
cd path/to/your/project
source .venv/bin/activate
python -m pip install mcp
```

## 3. Connect from Python

Create `mcd_mcp_client.py`:

```python
import json
import os
from pathlib import Path
from typing import Any

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


def default_mcd_mcp_command() -> str:
    if os.name == "nt":
        return str(Path.home() / ".cargo" / "bin" / "mcd-mcp.exe")
    return "mcd-mcp"


class McdMcpClient:
    def __init__(self, command: str | None = None):
        self.command = command or default_mcd_mcp_command()
        self._stdio = None
        self._session = None
        self.session: ClientSession | None = None

    async def __aenter__(self):
        server = StdioServerParameters(
            command=self.command,
            args=["--transport", "stdio"],
        )
        self._stdio = stdio_client(server)
        read, write = await self._stdio.__aenter__()
        self._session = ClientSession(read, write)
        self.session = await self._session.__aenter__()
        await self.session.initialize()
        return self

    async def __aexit__(self, exc_type, exc, tb):
        if self._session:
            await self._session.__aexit__(exc_type, exc, tb)
        if self._stdio:
            await self._stdio.__aexit__(exc_type, exc, tb)

    async def list_tools(self) -> list[dict[str, Any]]:
        if self.session is None:
            raise RuntimeError("MCP session is not initialized")
        result = await self.session.list_tools()
        return [
            {
                "name": tool.name,
                "description": tool.description or "",
                "input_schema": tool.inputSchema,
            }
            for tool in result.tools
        ]

    async def call_tool(self, name: str, arguments: dict[str, Any]) -> Any:
        if self.session is None:
            raise RuntimeError("MCP session is not initialized")
        result = await self.session.call_tool(name, arguments=arguments)

        structured = getattr(result, "structured_content", None)
        if structured is not None:
            return structured

        text = "\n".join(
            part.text
            for part in result.content
            if getattr(part, "type", None) == "text"
        )
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text
```

Test it with `test_mcd_mcp.py`:

```python
import asyncio

from mcd_mcp_client import McdMcpClient


async def main():
    async with McdMcpClient() as mcd:
        tools = await mcd.list_tools()
        print("Available MCD tools:", [tool["name"] for tool in tools])

        result = await mcd.call_tool(
            "mcd_search",
            {
                "path": r"C:\path\to\document.mcd",
                "query": "thermal_limit_deg_c coolant",
                "limit": 5,
            },
        )
        print(result)


asyncio.run(main())
```

Run:

```bat
python test_mcd_mcp.py
```

## 4. Connect It to Your Model

Your model does not talk to `.mcd` files directly. Your harness should:

1. Start `McdMcpClient`.
2. Call `list_tools()`.
3. Convert those tool definitions into your model provider's tool format.
4. Send the tools with the user's prompt.
5. When the model requests a tool call, run `call_tool(name, arguments)`.
6. Send the tool result back to the model.
7. Let the model write the final answer.

For MCD documents, a good default tool sequence is:

```text
mcd_validate -> mcd_agent_context -> mcd_search -> mcd_query
```

Use `mcd_search` for document passages and schema discovery. Use `mcd_query` for
exact table rows, filters, joins, aggregates, and ordering.

## 5. Useful First Calls

Validate a package:

```python
await mcd.call_tool("mcd_validate", {"path": r"C:\path\to\document.mcd"})
```

Get document structure:

```python
await mcd.call_tool("mcd_agent_context", {"path": r"C:\path\to\document.mcd"})
```

Search content and metadata:

```python
await mcd.call_tool(
    "mcd_search",
    {
        "path": r"C:\path\to\document.mcd",
        "query": "variant_id coolant",
        "limit": 5,
    },
)
```

Query table data:

```python
await mcd.call_tool(
    "mcd_query",
    {
        "path": r"C:\path\to\document.mcd",
        "sql": "select table_id, column_name, type from mcd_columns",
        "format": "json",
    },
)
```

## 6. Troubleshooting

If `mcd-mcp` is not found, use the full path:

```text
C:\Users\<you>\.cargo\bin\mcd-mcp.exe
```

If `cargo install` fails with `link.exe`, use the Windows prebuilt zip from step
1. The prebuilt binary does not require a local Rust compiler or Visual Studio
linker.

If tool calls fail because a package path is not found, pass an absolute path to
the `.mcd` file.
