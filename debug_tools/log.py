import asyncio

CURRENT_VERSION=1
async def handleMessage(reader, writer):
  version_bytes = await reader.readline()
  version = int(version_bytes.decode())
  if CURRENT_VERSION != version:
      print("Error version mismatch")
      #IDK what to do here 
      return
  message_bytes = await reader.readline()
  print(message_bytes.decode())

async def main():
    path = "/tmp/debug.socket"

    server = await asyncio.start_unix_server(handleMessage, path=path)
    print(f"Starting server on path: {path}")
    async with server:
        await server.serve_forever()
    
if __name__ == "__main__":
    asyncio.run(main()) 
