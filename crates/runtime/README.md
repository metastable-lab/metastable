## Runtime Architecture 

- Environment: Staging, Production, Sandbox
- Runtime Sequencing: 
    - Priori User Module (Authentication etc.)
    - Character Module 
    - ToolCall Module
    - InjectMemory Module 
    - RUN with <Request, Response>
    - ToolCall Execution
    - Postiori User Module (Billing, Usage Report, Update Memory etc.)

- MODE: 
    - SUSA - Single User Single Agent
    - SUMA - Single User Multiple Agent
    - MUSA - Multiple User Single Agent