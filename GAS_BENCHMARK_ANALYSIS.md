# Gas Cost Benchmark Analysis: Vector vs Map-Based Storage

## Executive Summary

This document provides a theoretical analysis of gas cost improvements achieved by migrating from vector-based to map-based storage patterns in the Zicket smart contracts.

## ⚠️ Important Disclaimer

**All gas costs in this document are THEORETICAL ESTIMATES** based on algorithmic complexity analysis. These are NOT measured results. Actual gas costs must be validated through testnet deployment with Soroban budget tracking enabled.

**To obtain verified benchmarks:**
1. Deploy contracts to Soroban testnet
2. Enable budget tracking: `env.budget().reset_default()`
3. Execute operations with varying collection sizes
4. Capture CPU and memory instruction counts
5. Document soroban-sdk version, protocol version, and test conditions

## Methodology

Theoretical gas cost analysis is based on:
1. **Serialization overhead**: Cost of encoding/decoding data structures
2. **Storage operations**: Cost of read/write operations
3. **Computational complexity**: O(n) vs O(1) operations

**Note**: Actual Soroban gas costs include protocol fees, host function overhead, and Wasm execution costs not captured in this theoretical model.

## Vector-Based Storage (Legacy)

### Storage Pattern
```rust
DataKey::OwnerTickets(Address) → Vec<u64>
```

### Operations Breakdown

#### Mint Ticket (Add to Vector)
1. **Read**: Deserialize entire vector from storage
2. **Modify**: Append new ticket ID to vector
3. **Write**: Serialize and write entire vector back

**Cost Growth**: O(n) where n = number of tickets owned

#### Transfer Ticket
1. **Read old owner**: Deserialize old owner's vector
2. **Remove from old**: Find and remove ticket ID
3. **Write old owner**: Serialize and write modified vector
4. **Read new owner**: Deserialize new owner's vector
5. **Add to new**: Append ticket ID
6. **Write new owner**: Serialize and write modified vector

**Cost Growth**: O(n₁ + n₂) where n₁ = old owner tickets, n₂ = new owner tickets

### Theoretical Gas Costs (Estimated, Unverified)

| Operation | User has 1 ticket | User has 10 tickets | User has 100 tickets | User has 1000 tickets |
|-----------|------------------|---------------------|---------------------|----------------------|
| Mint      | ~5,000 gas (est.) | ~7,000 gas (est.) | ~50,000 gas (est.) | ~500,000 gas (est.) |
| Transfer  | ~8,000 gas (est.) | ~12,000 gas (est.) | ~80,000 gas (est.) | ~800,000 gas (est.) |
| Check ownership | ~3,000 gas (est.) | ~4,500 gas (est.) | ~30,000 gas (est.) | ~300,000 gas (est.) |

## Map-Based Storage (New)

### Storage Pattern
```rust
DataKey::OwnerTicket(Address, u64) → bool
```

### Operations Breakdown

#### Mint Ticket
1. **Write**: Set single boolean flag

**Cost Growth**: O(1) - constant regardless of ticket count

#### Transfer Ticket
1. **Delete old owner**: Remove single key
2. **Write new owner**: Set single boolean flag

**Cost Growth**: O(1) - constant regardless of ticket count

### Theoretical Gas Costs (Estimated, Unverified)

| Operation | User has 1 ticket | User has 10 tickets | User has 100 tickets | User has 1000 tickets |
|-----------|------------------|---------------------|---------------------|----------------------|
| Mint      | ~5,000 gas (est.) | ~5,000 gas (est.) | ~5,000 gas (est.) | ~5,000 gas (est.) |
| Transfer  | ~6,000 gas (est.) | ~6,000 gas (est.) | ~6,000 gas (est.) | ~6,000 gas (est.) |
| Check ownership | ~2,000 gas (est.) | ~2,000 gas (est.) | ~2,000 gas (est.) | ~2,000 gas (est.) |

## Improvement Analysis

### Gas Savings by Operation

#### Mint Ticket
- **1 ticket**: 0% savings (both ~5K)
- **10 tickets**: 29% savings (7K → 5K)
- **100 tickets**: 90% savings (50K → 5K)
- **1000 tickets**: 99% savings (500K → 5K)

#### Transfer Ticket
- **1 ticket**: 25% savings (8K → 6K)
- **10 tickets**: 50% savings (12K → 6K)
- **100 tickets**: 92.5% savings (80K → 6K)
- **1000 tickets**: 99.25% savings (800K → 6K)

#### Check Ownership
- **1 ticket**: 33% savings (3K → 2K)
- **10 tickets**: 55% savings (4.5K → 2K)
- **100 tickets**: 93% savings (30K → 2K)
- **1000 tickets**: 99.3% savings (300K → 2K)

## Real-World Impact

### Scenario 1: Concert Venue (10,000 tickets)
**Average tickets per user**: 4 tickets

- **Mint cost reduction**: ~60% (20K → 5K gas per mint)
- **Transfer cost reduction**: ~70% (20K → 6K gas per transfer)
- **Total event savings**: ~6.5M gas units

### Scenario 2: Sports Season Tickets (500 tickets)
**Average tickets per user**: 20 tickets (season pass)

- **Mint cost reduction**: ~85% (100K → 5K gas per mint)
- **Transfer cost reduction**: ~95% (120K → 6K gas per transfer)
- **Frequent trader savings**: Up to 95% on all operations

### Scenario 3: High-Volume Collector (1000+ tickets)
**Power user collecting many events**

- **Mint cost reduction**: ~99% (500K → 5K gas per mint)
- **Transfer cost reduction**: ~99% (800K → 6K gas per transfer)
- **Economic viability**: Operations that were prohibitively expensive become affordable

## Serialization Overhead Analysis

### Vector Serialization Cost
```
Cost = Base + (Element_Size × Number_of_Elements)
     = 500 + (64 bits × n) / 8
     = 500 + (8n) gas units
```

### Boolean Serialization Cost
```
Cost = Base + Single_Bool
     = 500 + 1
     = 501 gas units (constant)
```

## Memory Access Patterns

### Vector Pattern (Cache-Unfriendly)
- **Full vector must be loaded**: Even for single element access
- **Memory footprint**: Grows linearly with vector size
- **Cache misses**: Increase with vector size

### Map Pattern (Cache-Friendly)
- **Single key lookup**: Only requested data loaded
- **Memory footprint**: Constant per operation
- **Cache misses**: Minimal, predictable

## Blockchain Storage Cost

### Vector Storage Cost
```
Storage_Cost = Key_Size + Vector_Header + (Item_Size × Count)
             = 32 bytes + 8 bytes + (8 bytes × n)
             = 40 + 8n bytes
```

**For 100 tickets**: 840 bytes per storage slot

### Map Storage Cost
```
Storage_Cost = Key_Size + Value_Size
             = (32 + 8) bytes + 1 byte
             = 41 bytes per relationship
```

**For 100 tickets**: 4,100 bytes total (100 separate keys)

**Note**: While total storage is higher, per-operation cost is dramatically lower due to avoiding vector serialization.

## Scalability Considerations

### Vector Approach Limits
- **Practical limit**: ~100-200 tickets per owner before gas costs become prohibitive
- **Network impact**: High gas spikes during popular events
- **User experience**: Degraded performance for active users

### Map Approach Advantages
- **No practical limit**: Constant cost regardless of collection size
- **Network impact**: Predictable, stable gas costs
- **User experience**: Consistent performance for all users

## Migration Cost-Benefit

### One-Time Migration Cost
- **Per ticket**: ~10K gas to migrate from vector to map
- **Amortization**: Savings recovered after 2-3 operations

### Break-Even Point
- **Low-activity user** (1-2 ops/month): 6 months
- **Medium-activity user** (5-10 ops/month): 1 month
- **High-activity user** (20+ ops/month): 1 week

## Conclusion

The migration from vector-based to map-based storage is expected to provide:

1. **Potential benefits** for users with multiple tickets (pending verification)
2. **Long-term scalability** through O(1) operations
3. **More predictable algorithmic costs** for operations
4. **Improved economic viability** for high-volume users (pending gas measurement)

### Recommended Actions

1. ✅ **Implement map-based storage** (Completed)
2. 🔄 **Deploy to testnet** for real gas measurements (CRITICAL NEXT STEP)
3. ⏳ **Measure and document** actual gas costs with budget tracking
4. ⏳ **Compare measured results** against these theoretical estimates
5. ⏳ **Publish verified benchmarks** with SDK/protocol versions
6. ⏳ **Monitor gas costs** in production
7. ⏳ **Gradual migration** of existing users

### Projected Platform Impact (Pending Verification)

- **Estimated 70-90% reduction** in gas costs for active users (theoretical)
- **More predictable transaction costs** through O(1) operations
- **Theoretical scalability** to support 10x more tickets per user
- **Expected reduction** in network congestion during peak events

**⚠️ These projections must be validated through testnet measurements before making production claims.**

## References

- Vector-to-Map Migration Guide: [VECTOR_TO_MAP_MIGRATION.md](./VECTOR_TO_MAP_MIGRATION.md)
- Soroban Storage Documentation: https://soroban.stellar.org/docs/learn/storage
- Smart Contract Gas Optimization: https://soroban.stellar.org/docs/learn/optimization
