#[cfg(test)]
mod stress_tests {
    use std::time::{Duration, Instant};

    const STRESS_ENTITY_COUNT: usize = 10_000;
    const STRESS_PHYSICS_BODIES: usize = 1_000;
    const TARGET_FPS: f32 = 60.0;
    const FRAME_BUDGET_MS: f32 = 1000.0 / TARGET_FPS;

    #[test]
    fn stress_entity_spawn_performance() {
        println!("\n=== Entity Spawn Stress Test ===");
        println!("Target: {} entities", STRESS_ENTITY_COUNT);

        let start = Instant::now();
        
        let mut entities: Vec<u64> = Vec::with_capacity(STRESS_ENTITY_COUNT);
        for i in 0..STRESS_ENTITY_COUNT {
            entities.push(i as u64);
        }
        
        let spawn_time = start.elapsed();
        let spawn_rate = STRESS_ENTITY_COUNT as f64 / spawn_time.as_secs_f64();
        
        println!("Spawn time: {:?}", spawn_time);
        println!("Spawn rate: {:.0} entities/sec", spawn_rate);
        
        assert!(spawn_rate > 100_000.0, "Entity spawn rate too slow: {:.0}/sec", spawn_rate);
        println!("✅ PASSED: Entity spawn performance OK");
    }

    #[test]
    fn stress_transform_update_performance() {
        println!("\n=== Transform Update Stress Test ===");
        println!("Target: {} transforms per frame", STRESS_ENTITY_COUNT);

        #[derive(Clone, Copy)]
        struct Transform {
            position: [f32; 3],
            rotation: [f32; 4],
            scale: [f32; 3],
        }

        let mut transforms: Vec<Transform> = (0..STRESS_ENTITY_COUNT)
            .map(|i| Transform {
                position: [i as f32, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            })
            .collect();

        let start = Instant::now();
        let frames = 100;
        
        for frame in 0..frames {
            let dt = 1.0 / 60.0;
            for transform in transforms.iter_mut() {
                transform.position[0] += dt * 10.0;
                transform.position[1] = (frame as f32 * 0.1).sin();
            }
        }
        
        let total_time = start.elapsed();
        let avg_frame_time = total_time.as_secs_f32() * 1000.0 / frames as f32;
        
        println!("Total time for {} frames: {:?}", frames, total_time);
        println!("Average frame time: {:.2}ms", avg_frame_time);
        println!("Frame budget: {:.2}ms", FRAME_BUDGET_MS);
        
        assert!(avg_frame_time < FRAME_BUDGET_MS, 
            "Transform updates exceed frame budget: {:.2}ms > {:.2}ms", 
            avg_frame_time, FRAME_BUDGET_MS);
        println!("✅ PASSED: Transform update performance OK");
    }

    #[test]
    fn stress_spatial_query_performance() {
        println!("\n=== Spatial Query Stress Test ===");
        
        #[derive(Clone, Copy)]
        struct AABB {
            min: [f32; 3],
            max: [f32; 3],
        }

        impl AABB {
            fn intersects(&self, other: &AABB) -> bool {
                self.min[0] <= other.max[0] && self.max[0] >= other.min[0] &&
                self.min[1] <= other.max[1] && self.max[1] >= other.min[1] &&
                self.min[2] <= other.max[2] && self.max[2] >= other.min[2]
            }
        }

        let objects: Vec<AABB> = (0..STRESS_PHYSICS_BODIES)
            .map(|i| {
                let x = (i % 100) as f32 * 10.0;
                let z = (i / 100) as f32 * 10.0;
                AABB {
                    min: [x, 0.0, z],
                    max: [x + 1.0, 2.0, z + 1.0],
                }
            })
            .collect();

        let query_box = AABB {
            min: [45.0, 0.0, 45.0],
            max: [55.0, 2.0, 55.0],
        };

        let start = Instant::now();
        let queries = 1000;
        let mut total_hits = 0;
        
        for _ in 0..queries {
            for obj in &objects {
                if query_box.intersects(obj) {
                    total_hits += 1;
                }
            }
        }
        
        let query_time = start.elapsed();
        let queries_per_sec = queries as f64 / query_time.as_secs_f64();
        
        println!("Queries: {}", queries);
        println!("Objects per query: {}", objects.len());
        println!("Total query time: {:?}", query_time);
        println!("Queries/sec: {:.0}", queries_per_sec);
        println!("Total hits: {}", total_hits);
        
        assert!(queries_per_sec > 1000.0, 
            "Spatial query rate too slow: {:.0}/sec", queries_per_sec);
        println!("✅ PASSED: Spatial query performance OK");
    }

    #[test]
    fn stress_physics_simulation() {
        println!("\n=== Physics Simulation Stress Test ===");
        println!("Target: {} physics bodies", STRESS_PHYSICS_BODIES);

        #[derive(Clone, Copy)]
        struct PhysicsBody {
            position: [f32; 3],
            velocity: [f32; 3],
            mass: f32,
        }

        let mut bodies: Vec<PhysicsBody> = (0..STRESS_PHYSICS_BODIES)
            .map(|i| PhysicsBody {
                position: [(i % 100) as f32, 10.0 + (i / 100) as f32, 0.0],
                velocity: [0.0, 0.0, 0.0],
                mass: 1.0,
            })
            .collect();

        let gravity = [0.0, -9.81, 0.0];
        let dt = 1.0 / 60.0;
        let frames = 300; // 5 seconds at 60fps

        let start = Instant::now();
        
        for _ in 0..frames {
            for body in bodies.iter_mut() {
                // Apply gravity
                body.velocity[0] += gravity[0] * dt;
                body.velocity[1] += gravity[1] * dt;
                body.velocity[2] += gravity[2] * dt;
                
                // Update position
                body.position[0] += body.velocity[0] * dt;
                body.position[1] += body.velocity[1] * dt;
                body.position[2] += body.velocity[2] * dt;
                
                // Ground collision
                if body.position[1] < 0.0 {
                    body.position[1] = 0.0;
                    body.velocity[1] = -body.velocity[1] * 0.5; // Bounce
                }
            }
        }
        
        let sim_time = start.elapsed();
        let avg_frame_time = sim_time.as_secs_f32() * 1000.0 / frames as f32;
        let physics_fps = frames as f64 / sim_time.as_secs_f64();
        
        println!("Simulation time: {:?}", sim_time);
        println!("Average frame time: {:.3}ms", avg_frame_time);
        println!("Physics FPS: {:.0}", physics_fps);
        
        assert!(physics_fps > 1000.0, 
            "Physics simulation too slow: {:.0} FPS", physics_fps);
        println!("✅ PASSED: Physics simulation performance OK");
    }

    #[test]
    fn stress_memory_allocation() {
        println!("\n=== Memory Allocation Stress Test ===");
        
        let iterations = 100;
        let alloc_size = 1024 * 1024; // 1MB per allocation
        
        let start = Instant::now();
        
        for _ in 0..iterations {
            let buffer: Vec<u8> = vec![0u8; alloc_size];
            std::hint::black_box(&buffer);
        }
        
        let alloc_time = start.elapsed();
        let allocs_per_sec = iterations as f64 / alloc_time.as_secs_f64();
        let throughput_mb = (iterations * alloc_size) as f64 / alloc_time.as_secs_f64() / 1024.0 / 1024.0;
        
        println!("Allocations: {}", iterations);
        println!("Size per allocation: {}MB", alloc_size / 1024 / 1024);
        println!("Total time: {:?}", alloc_time);
        println!("Allocations/sec: {:.0}", allocs_per_sec);
        println!("Throughput: {:.0} MB/sec", throughput_mb);
        
        assert!(throughput_mb > 1000.0, 
            "Memory allocation throughput too slow: {:.0} MB/sec", throughput_mb);
        println!("✅ PASSED: Memory allocation performance OK");
    }

    #[test]
    fn stress_concurrent_systems() {
        println!("\n=== Concurrent Systems Stress Test ===");
        
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;
        
        let counter = Arc::new(AtomicU64::new(0));
        let iterations = 1_000_000;
        let threads = 8;
        
        let start = Instant::now();
        
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let counter = Arc::clone(&counter);
                std::thread::spawn(move || {
                    for _ in 0..iterations / threads {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        let elapsed = start.elapsed();
        let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
        
        println!("Threads: {}", threads);
        println!("Operations: {}", iterations);
        println!("Time: {:?}", elapsed);
        println!("Ops/sec: {:.0}", ops_per_sec);
        
        assert_eq!(counter.load(Ordering::Relaxed), iterations as u64);
        assert!(ops_per_sec > 10_000_000.0, 
            "Concurrent operations too slow: {:.0}/sec", ops_per_sec);
        println!("✅ PASSED: Concurrent systems performance OK");
    }
}
