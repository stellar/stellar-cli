# contacts-mcp-server

MCP Server for Soroban Smart Contract

This server provides a REST API interface for interacting with a Soroban smart contract. It was automatically generated using the Soroban CLI.

## Setup

1. Install dependencies:
```bash
npm install
```

2. Copy the environment file and configure it:
```bash
cp .env.example .env
```

Edit the `.env` file and set your configuration values:
- `NETWORK`: The Stellar network to use (testnet/public)
- `NETWORK_PASSPHRASE`: The network passphrase
- `RPC_URL`: Soroban RPC server URL
- `CONTRACT_ID`: Your contract ID
- `PORT`: Server port (default: 3000)
- `HOST`: Server host (default: localhost)
- `ADMIN_KEY`: (Optional) Key for protected endpoints
- `RATE_LIMIT_WINDOW`: (Optional) Rate limiting window
- `RATE_LIMIT_MAX`: (Optional) Maximum requests per window

## Development

Run in development mode with hot reloading:
```bash
npm run dev
```

## Production

Build and run in production:
```bash
npm run build
npm start
```

## API Endpoints

### Health Check
- `GET /health`: Check server status

### Contract Methods
The following endpoints are automatically generated based on your contract's methods:

INSERT_ENDPOINTS_HERE

Each endpoint accepts POST requests with a JSON body containing the method parameters.

## Error Handling

All endpoints return:
- 200: Successful operation
- 400: Invalid parameters
- 500: Server/contract error

Error responses include an `error` field with a description of what went wrong.

## Rate Limiting

If configured in the `.env` file, the server implements rate limiting per IP address.

## Security

- CORS is enabled and can be configured in `src/index.ts`
- Optional admin key for protected endpoints
- Rate limiting to prevent abuse
- Input validation for all endpoints

## Contributing

This is an auto-generated server. If you need to modify the contract interface, it's recommended to regenerate the server using the Soroban CLI rather than modifying the generated code directly. 