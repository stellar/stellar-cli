import { FastifyInstance } from 'fastify';
import { Contract, Server } from '@stellar/stellar-sdk';

export function registerRoutes(
  app: FastifyInstance,
  contract: Contract,
  server: Server
): void {
  // Health check endpoint
  app.get('/health', async () => {
    return { status: 'ok' };
  });

  // Contract endpoints will be dynamically generated here based on the contract spec
  // Example:
  /*
  app.post('/method_name', async (request, reply) => {
    try {
      const result = await contract.call('method_name', [
        // parameters from request.body
      ]);
      return result;
    } catch (error) {
      reply.status(500).send({ error: error.message });
    }
  });
  */
} 