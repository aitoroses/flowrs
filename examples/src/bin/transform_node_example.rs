use async_trait::async_trait;
use flowrs_transform::{
    TransformContext, TransformNode, create_transform_node, to_lifecycle_node,
};
use flowrs_core::{
    DefaultAction, FlowrsError, Node, NodeOutcome, Workflow,
    lifecycle::LifecycleNodeAdapter,
};
use std::{error::Error, fmt};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Custom error type for our transform nodes
#[derive(Debug)]
struct TextProcessingError(String);

impl fmt::Display for TextProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Text processing error: {}", self.0)
    }
}

impl Error for TextProcessingError {}

// Make our custom error convertible to FlowrsError
impl From<TextProcessingError> for FlowrsError {
    fn from(err: TextProcessingError) -> Self {
        FlowrsError::node_execution("transform_node", &err.0)
    }
}

/// A struct-based transform node that processes text
struct TextTransformer;

#[async_trait]
impl TransformNode<String, String, TextProcessingError> for TextTransformer {
    async fn prep(&self, input: String) -> Result<String, TextProcessingError> {
        // Validation step
        if input.trim().is_empty() {
            return Err(TextProcessingError("Input text cannot be empty".to_string()));
        }
        
        info!("Preparing text transformation: '{}' (length: {})", input, input.len());
        Ok(input)
    }

    async fn exec(&self, input: String) -> Result<String, TextProcessingError> {
        // Main transformation logic
        let result = input.to_uppercase();
        info!("Transformed text to uppercase: '{}'", result);
        Ok(result)
    }

    async fn post(&self, output: String) -> Result<String, TextProcessingError> {
        // Post-processing step
        let result = format!("PROCESSED: {}", output);
        info!("Post-processed result: '{}'", result);
        Ok(result)
    }
}

/// A more complex example showing text analysis using transform node
///
/// This node counts words and characters in a text input
struct TextAnalyzer;

#[derive(Debug, Clone)]
struct TextStats {
    word_count: usize,
    character_count: usize,
    has_numbers: bool,
}

#[async_trait]
impl TransformNode<String, TextStats, TextProcessingError> for TextAnalyzer {
    async fn prep(&self, input: String) -> Result<String, TextProcessingError> {
        // Check if the input is valid
        if input.trim().is_empty() {
            return Err(TextProcessingError("Input text cannot be empty".to_string()));
        }
        
        info!("Preparing text analysis for: '{}'", input);
        Ok(input)
    }

    async fn exec(&self, input: String) -> Result<TextStats, TextProcessingError> {
        // Count words and characters
        let words = input.split_whitespace().count();
        let chars = input.chars().count();
        let has_numbers = input.chars().any(|c| c.is_numeric());
        
        let stats = TextStats {
            word_count: words,
            character_count: chars,
            has_numbers,
        };
        
        info!("Analysis results: {} words, {} characters, contains numbers: {}", 
              stats.word_count, stats.character_count, stats.has_numbers);
        
        Ok(stats)
    }

    async fn post(&self, output: TextStats) -> Result<TextStats, TextProcessingError> {
        // No modification needed in post-processing
        info!("Finalized text analysis: {:?}", output);
        Ok(output)
    }
}

/// Example of creating a transform node from closures
fn create_greeting_transformer() -> impl TransformNode<String, String, TextProcessingError> + 'static {
    // Use the BoxFuture type from tokio instead of futures
    create_transform_node(
        // prep closure
        |name: String| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, TextProcessingError>> + Send>> {
            Box::pin(async move {
                if name.trim().is_empty() {
                    return Err(TextProcessingError("Name cannot be empty".to_string()));
                }
                info!("Preparing greeting for name: {}", name);
                Ok(name)
            })
        },
        // exec closure
        |name: String| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, TextProcessingError>> + Send>> {
            Box::pin(async move {
                let greeting = format!("Hello, {}!", name);
                info!("Created greeting: {}", greeting);
                Ok(greeting)
            })
        },
        // post closure
        |greeting: String| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, TextProcessingError>> + Send>> {
            Box::pin(async move {
                let final_greeting = format!("{} Have a great day!", greeting);
                info!("Finalized greeting: {}", final_greeting);
                Ok(final_greeting)
            })
        }
    )
}

/// Custom step that uses the output of a previous transform node
struct GreetingPrinter {
    id: String,
}

impl GreetingPrinter {
    fn new() -> Self {
        Self {
            id: "greeting-printer".to_string(),
        }
    }
}

#[async_trait]
impl Node<TransformContext<String>, DefaultAction> for GreetingPrinter {
    // Match the output type of the workflow (String)
    type Output = String;

    fn id(&self) -> String {
        self.id.clone()
    }

    async fn process(&self, ctx: &mut TransformContext<String>) -> Result<NodeOutcome<Self::Output, DefaultAction>, FlowrsError> {
        // Print a nicely formatted message
        println!("\n=== Final Message ===");
        println!("{}", ctx.input);
        println!("====================\n");
        
        // Return the message as the output and route to the next node
        Ok(NodeOutcome::RouteToAction(DefaultAction::Default))
    }
}

/// A simple output node that captures the final result
struct OutputNode<T: Clone + Send + Sync + 'static> {
    id: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone + Send + Sync + 'static> OutputNode<T> {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T> Node<TransformContext<T>, DefaultAction> for OutputNode<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Output = T;

    fn id(&self) -> String {
        self.id.clone()
    }

    async fn process(&self, ctx: &mut TransformContext<T>) -> Result<NodeOutcome<Self::Output, DefaultAction>, FlowrsError> {
        // Just capture the input value and return Success
        Ok(NodeOutcome::Success(ctx.input.clone()))
    }
}

/// Example 1: Simple text transformation from "hello world" to uppercase
async fn run_text_transformation_example() -> Result<(), Box<dyn Error>> {
    info!("=== Example 1: Simple Text Transformation ===");
    
    // Create the transform node
    let text_transformer = TextTransformer;
    
    // Convert to lifecycle node for use with Workflow
    let lifecycle_node = to_lifecycle_node::<_, String, String, TextProcessingError, DefaultAction>(text_transformer);
    
    // Create adapter from LifecycleNode to Node for use with Workflow
    let adapter = LifecycleNodeAdapter::new(lifecycle_node);
    let node_id = adapter.id();
    
    // Create a final output node to capture the result
    let output_node = OutputNode::<String>::new("output-node");
    let output_id = output_node.id();
    
    // Create workflow 
    let mut workflow = Workflow::new(adapter);
    
    // Add output node and connect it
    workflow.add_node(output_node);
    
    // Connect with DefaultAction::Default as the action
    workflow.connect(&node_id, DefaultAction::Default, &output_id);
    
    info!("Workflow structure:");
    info!("Starting node: {}", node_id);
    info!("Output node: {}", output_id);
    info!("Connection: {} -[Default]-> {}", node_id, output_id);
    
    // Create context with input
    let mut ctx = TransformContext::new("hello world".to_string());
    
    // Execute workflow
    let result = workflow.execute(&mut ctx).await?;
    
    println!("Text transformation result: {}", result);
    Ok(())
}

/// Example 2: Text analysis to count words, characters and check for numbers
async fn run_text_analysis_example() -> Result<(), Box<dyn Error>> {
    info!("=== Example 2: Text Analysis ===");
    
    // Create the transform node
    let text_analyzer = TextAnalyzer;
    
    // Convert to lifecycle node
    let lifecycle_node = to_lifecycle_node::<_, String, TextStats, TextProcessingError, DefaultAction>(text_analyzer);
    
    // Create adapter for Workflow
    let adapter = LifecycleNodeAdapter::new(lifecycle_node);
    let node_id = adapter.id();
    
    // Create workflow that works with TransformContext<String> context
    // and produces TextStats output
    let mut workflow = Workflow::<TransformContext<String>, DefaultAction, TextStats>::new(adapter);
    
    // Create a wrapper node that can access TextStats from a TransformContext<String>
    struct TextStatsOutputNode {
        id: String,
    }
    
    impl TextStatsOutputNode {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
            }
        }
    }
    
    #[async_trait]
    impl Node<TransformContext<String>, DefaultAction> for TextStatsOutputNode {
        type Output = TextStats;
        
        fn id(&self) -> String {
            self.id.clone()
        }
        
        async fn process(&self, ctx: &mut TransformContext<String>) -> Result<NodeOutcome<Self::Output, DefaultAction>, FlowrsError> {
            // Since we can't directly access the TextStats result, we'll create a dummy
            // This is only for the example. In a real workflow, you would have access
            // to the TextStats from a previous node or from the workflow context
            let stats = TextStats {
                word_count: ctx.input.split_whitespace().count(),
                character_count: ctx.input.chars().count(),
                has_numbers: ctx.input.chars().any(|c| c.is_numeric()),
            };
            
            Ok(NodeOutcome::Success(stats))
        }
    }
    
    // Create output node with the specific ID
    let output_node = TextStatsOutputNode::new("output-node");
    let output_id = output_node.id();
    
    // Add output node and connect it
    workflow.add_node(output_node);
    
    // Connect with DefaultAction::Default as the action 
    workflow.connect(&node_id, DefaultAction::Default, &output_id);
    
    // Debugging information
    info!("Workflow structure:");
    info!("Starting node: {}", node_id);
    info!("Output node: {}", output_id);
    info!("Connection: {} -[Default]-> {}", node_id, output_id);
    
    // Create context with input
    let mut ctx = TransformContext::new("This is a sample text with 123 numbers.".to_string());
    
    // Execute workflow
    let stats = workflow.execute(&mut ctx).await?;
    
    println!("Text Analysis Results:");
    println!("- Word count: {}", stats.word_count);
    println!("- Character count: {}", stats.character_count);
    println!("- Contains numbers: {}", stats.has_numbers);
    
    Ok(())
}

/// Example 3: Using closures with a three-node workflow
async fn run_closure_transform_example() -> Result<(), Box<dyn Error>> {
    info!("=== Example 3: Using Closures with Greeting Printer ===");
    
    // Create transform node from closures
    let greeting_transformer = create_greeting_transformer();
    
    // Convert to lifecycle node with a specific ID for the workflow
    let transformer_id = "greeting-transformer".to_string();
    
    // Create a custom lifecycle node with the specific ID
    let lifecycle_node = {
        // First, convert the transform node to a lifecycle node
        let adapter = to_lifecycle_node::<_, String, String, TextProcessingError, DefaultAction>(greeting_transformer);
        
        // Then create a struct that delegates to it but with a custom ID
        #[derive(Debug)]
        struct CustomIdLifecycleNode<L, C, A> {
            inner: L,
            custom_id: String,
            _phantom: std::marker::PhantomData<(C, A)>,
        }
        
        #[async_trait]
        impl<L, C, A> flowrs_core::LifecycleNode<C, A> for CustomIdLifecycleNode<L, C, A>
        where
            L: flowrs_core::LifecycleNode<C, A> + Send + Sync,
            C: Send + Sync + 'static,
            A: flowrs_core::ActionType + Send + Sync + 'static,
            L::PrepOutput: Clone + Send + Sync + 'static,
            L::ExecOutput: Clone + Send + Sync + 'static,
        {
            type PrepOutput = L::PrepOutput;
            type ExecOutput = L::ExecOutput;
            
            fn id(&self) -> String {
                self.custom_id.clone()
            }
            
            async fn prep(&self, ctx: &mut C) -> Result<Self::PrepOutput, FlowrsError> {
                self.inner.prep(ctx).await
            }
            
            async fn exec(&self, prep_result: Self::PrepOutput) -> Result<Self::ExecOutput, FlowrsError> {
                self.inner.exec(prep_result).await
            }
            
            async fn post(
                &self,
                prep_result: Self::PrepOutput,
                exec_result: Self::ExecOutput,
                ctx: &mut C,
            ) -> Result<A, FlowrsError> {
                self.inner.post(prep_result, exec_result, ctx).await
            }
        }
        
        CustomIdLifecycleNode {
            inner: adapter,
            custom_id: transformer_id.clone(),
            _phantom: std::marker::PhantomData,
        }
    };
    
    // Create adapter for use with Workflow
    let adapter = LifecycleNodeAdapter::new(lifecycle_node);
    
    // Create workflow with the greeting transformer as the entry node
    let mut workflow = Workflow::new(adapter);
    
    // Create a printer with a known ID
    let printer = GreetingPrinter::new();
    let printer_id = printer.id.clone(); // Save the ID before moving
    
    // Create final output node
    let output_node = OutputNode::<String>::new("output-node");
    let output_id = output_node.id();
    
    // Add the printer and output nodes
    workflow.add_node(printer);
    workflow.add_node(output_node);
    
    // Connect with DefaultAction::Default as the action
    workflow.connect(&transformer_id, DefaultAction::Default, &printer_id);
    workflow.connect(&printer_id, DefaultAction::Default, &output_id);
    
    // Debugging information
    info!("Workflow structure:");
    info!("Starting node: {}", transformer_id);
    info!("Printer node: {}", printer_id);
    info!("Output node: {}", output_id);
    info!("Connection 1: {} -[Default]-> {}", transformer_id, printer_id);
    info!("Connection 2: {} -[Default]-> {}", printer_id, output_id);
    
    // Create context with input
    let mut ctx = TransformContext::new("Rust Developer".to_string());
    
    // Execute workflow
    let result = workflow.execute(&mut ctx).await?;
    
    // This should print the final greeting
    println!("Final workflow result: {}", result);
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Transform Node Example");
    
    // Run the examples
    run_text_transformation_example().await?;
    run_text_analysis_example().await?;
    run_closure_transform_example().await?;
    
    info!("Transform Node Example completed successfully");
    Ok(())
} 