import { close, DataType, define, open } from 'ffi-rs'
import * as path from 'path'

// Path to the compiled Rust library
// Before running this example run cargo run build --release --example basic to generate the libconduit_example_lib.so file
const libPath = path.join(__dirname, '../../../target/release/libconduit_example_lib.so')

console.log('Loading library from:', libPath)

// Open the library
try {
    open({
        library: 'conduit_example_lib',
        path: libPath,
    })
    console.log('Successfully loaded conduit_example_lib')
} catch (error) {
    console.error('Failed to load conduit_example_lib:', error)
    process.exit(1)
}

// Define the FFI interface
const conduitFunctions = define({
    conduit_engine_new: {
        library: 'conduit_example_lib',
        retType: DataType.External, // Using External for pointer return type
        paramsType: [],
    },
    conduit_engine_free: {
        library: 'conduit_example_lib',
        retType: DataType.Void,
        paramsType: [ DataType.External ], // Using External for pointer parameter
    },
    conduit_run_pipeline: {
        library: 'conduit_example_lib',
        retType: DataType.Boolean,
        paramsType: [ DataType.External, DataType.String ],
    },
    conduit_free_string: {
        library: 'conduit_example_lib',
        retType: DataType.Void,
        paramsType: [ DataType.External ],
    },
})

// Create a TypeScript wrapper class
export class ConduitEngine {
    private engine: any

    constructor() {
        console.log('Creating engine instance...')
        this.engine = conduitFunctions.conduit_engine_new([])
        if (!this.engine) {
            throw new Error('Failed to create Conduit engine')
        }
        console.log('Engine created successfully')
    }

    runPipeline(pipelineCode: string): boolean {
        if (!this.engine) {
            throw new Error('Engine has been destroyed')
        }

        console.log('Running pipeline with code:', pipelineCode)
        return conduitFunctions.conduit_run_pipeline([ this.engine, pipelineCode ])
    }

    destroy(): void {
        if (this.engine) {
            console.log('Destroying engine...')
            conduitFunctions.conduit_engine_free([ this.engine ])
            this.engine = null
            console.log('Engine destroyed')
            console.log('Closing library...')
            close('libconduit_example_lib')
            console.log('Library closed')
        }
    }
}