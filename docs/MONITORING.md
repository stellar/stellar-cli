# Monitoring and Tuning Guide for soroban-rpc

## Introduction

This document provides a comprehensive guide to monitoring and tuning soroban-rpc, a backend server that communicates using the jrpc (JSON-RPC) protocol over HTTP. To ensure high
availability, high performance, and efficient resource utilization, soroban-rpc incorporates various features like limiting concurrent requests, controlling execution times, and providing
warning and limiting mechanisms. This guide aims to help operators effectively monitor the server, detect potential issues, and apply tuning strategies to maintain optimal performance.

## Monitoring Metrics

To ensure the smooth operation of soroban-rpc, several key metrics should be monitored continuously:

1. **Global Inflight Requests (Concurrent HTTP Requests)**: Monitor the number of concurrent HTTP requests being enqueued at the HTTP endpoint. This metric is tracked via the
   `global_inflight_requests` gauge. If this number reaches the predefined limit, an HTTP 503 error is generated. This metric helps identify if the server is reaching its limit in handling
   incoming requests.

2. **Method-specific Inflight Requests (Concurrent JRPC Requests)**: Track the number of concurrent JRPC requests for each method using the `<method_name>_inflight_requests` gauge. This
   allows you to limit the workload of specific methods in case the server runs out of resources.

3. **HTTP Request Duration**: Monitor the duration taken to process each HTTP request. This metric helps identify if any requests are taking too long to process and may lead to potential
   performance issues. If the duration limit is reached, an HTTP 504 error is generated. The total number of warnings generated is tracked by the
   `global_request_execution_duration_threshold_warning` counter, and the number of terminated methods is tracked via the `global_request_execution_duration_threshold_limit` counter.

4. **Method-specific Execution Warnings and Limits**: Measure the execution time of each method and compare it against the predefined threshold. Track the execution warnings using the
   `<method_name>_execution_threshold_warning` counter and the execution limits using the `<method_name>_execution_threshold_limit` counter. These metrics help operators identify
   slow-performing methods and set execution limits to prevent resource exhaustion.

## Best Practices

Follow these best practices to maintain a stable and performant soroban-rpc deployment:

1. **Set Sensible Limits**: Determine appropriate limits for concurrent requests, method execution times, and HTTP request duration based on your server's resources and expected workload.
   Avoid overly restrictive limits that may hinder normal operations.

2. **Logging and Alerts**: The soroban-rpc comes ready with logging and metric endpoint, which reports operational status. On your end, develop the toolings that would allow you to be aware of these events. These toolings could be Grafana alerts, log scraping or similar tools.

3. **Load Testing**: Regularly conduct load testing to assess the server's performance under varying workloads. Use this data to adjust limits and execution times as needed.

4. **Scaling Strategies**: Plan scaling strategies for both vertical and horizontal scaling. Vertical scaling involves upgrading hardware resources like CPU, memory, and disk, while
   horizontal scaling uses HTTP-aware load balancers to distribute the load across multiple machines.

## Tuning Suggestions

When monitoring the resource utilization and identifying gradual increases in method execution times, consider the following tuning suggestions:

1. **Vertical Tuning**:

   - Increase CPU resources: Faster processors can reduce method execution times, improving overall performance.
   - Add Memory: Sufficient memory helps reduce disk I/O and can optimize processing times.
   - Use Faster Disk: SSDs or faster disk technologies can significantly improve I/O performance.

2. **Horizontal Tuning**:

   - Employ HTTP-Aware Load Balancers: Use load balancers that are aware of HTTP error codes and response times. This enables effective distribution of requests across multiple instances
     while considering their respective loads and response times.

3. **Quantitative Tuning**:
   - Adjust Concurrency Levels: Fine-tune the concurrency limits for specific methods based on their individual resource requirements and importance. This allows you to prioritize critical
     methods and prevent resource contention.
   - Limit Execution Times: Set appropriate execution time limits for methods, ensuring that no single method consumes excessive resources.
   - Divide and Conquer: Create several service performance groups, allowing a subset of the users to receive a favorable method execution times.
