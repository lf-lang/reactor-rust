initSidebarItems({"struct":[["AsyncCtx","A type that can affect the logical event queue to implement asynchronous physical actions. This is a “link” to the event system, from the outside world."],["Duration","A `Duration` type to represent a span of time, typically used for system timeouts."],["EventTag","The tag of an event."],["Instant","A measurement of a monotonically nondecreasing clock. Opaque and useful only with [`Duration`]."],["LogicalAction","A logical action."],["PhysicalActionRef","A reference to a physical action. This thing is cloneable and can be sent to async threads. The contained action reference is unique and protected by a lock. All operations on the action are"],["ReactionCtx","The context in which a reaction executes. Its API allows mutating the event queue of the scheduler. Only the interactions declared at assembly time are allowed."],["ReadablePort","A read-only reference to a port."],["ReadablePortBank","A read-only reference to a port bank."],["Timer","A timer is conceptually a logical action that may re-schedule itself periodically."],["WritablePort","A write-only reference to a port."],["WritablePortBank",""]],"type":[["unit","Alias for the unit type, so that it can be written without quotes in LF. Otherwise it needs to be written `{= () =}`. It is not camel-case as it is actually a primitive type."]]});