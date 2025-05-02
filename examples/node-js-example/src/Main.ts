import { ConduitEngine } from './Engine'

const engine = new ConduitEngine()

const pipeline = `
    resizer {
        source <- read_file {
            input <- "../conduit-example/cover.png"
        }
        width <- 512
        height <- 215
        output -> write_file {
            destination <- "../conduit-example/cover.smaller.png"
        }
    }
`

const success = engine.runPipeline(pipeline)

if (success) {
    console.log('Pipeline executed successfully!')
} else {
    console.error('Failed to execute pipeline')
}

engine.destroy()
