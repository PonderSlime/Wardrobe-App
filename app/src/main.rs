use eframe::egui::{self, accesskit::AriaCurrent::False};
use rfd::MessageDialogResult::No;
use std::{os::raw, time::Instant};
use std::{clone, fs};
use std::path::Path;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use image::{GenericImage, GenericImageView, RgbaImage};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: String,          // Unique ID
    pub category: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub style: String,
    pub season: String,
    pub occasion: String,
    pub material: String,
    pub fit: String,
    pub image_path: String,  // Path to the image on disk
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outfit {
    pub id: String,
    pub name: String,
    pub occasion: String,
    pub style: String,
    pub season: Vec<String>,                    // "season": []
    pub weather: serde_json::Value,             // "weather": {} -> holds arbitrary JSON object data
    pub items: Vec<String>,                     // Array of InventoryItem IDs
    pub collage_path: String,
    pub score: i32,
    pub user_rating: Option<f32>,               // Option handles null values
    pub liked: bool,
    pub favorite: bool,
    pub times_worn: u32,
    pub last_worn: Option<String>,              // Option handles null values
    pub reasoning: Vec<String>,                 // "reasoning": []
    pub created_at: String,
}

// 1. Define our Tabs
#[derive(PartialEq)]
enum Tab {
    Inventory,
    Outfits,
    AddItem,
}

// 2. Define the state for the "Add Item" loading sequence
enum AddItemState {
    Idle { error: Option<String> },
    Loading { start_time: Instant, file_path: String },
    ProcessingAI {
        receiver: std::sync::mpsc::Receiver<Result<InventoryItem, String>>,
    },
    ViewingDetails { 
        file_path: String, 
        texture: egui::TextureHandle // Store the loaded texture here
    },
}
enum OutfitState {
    Idle,
    Processing { 
        receiver: std::sync::mpsc::Receiver<Result<Outfit, String>> 
    },
}
// 3. Main App Struct
struct MyApp {
    active_tab: Tab,
    
    // Loaded Collections
    inventory_map: HashMap<String, InventoryItem>, // Quick lookup by ID
    inventory_list: Vec<InventoryItem>,            // Ordered list for the grid
    outfits_list: Vec<Outfit>,
    outfit_collage_cache: HashMap<String, egui::TextureHandle>,
    show_create_outfit_popup: bool,
    outfit_prompt: String,
    outfit_state: OutfitState,
    // UI Selection Tracking (Now tracking by string IDs)
    selected_inventory_item_id: Option<String>,
    selected_outfit_id: Option<String>,

    navigation_source_outfit_id: Option<String>,

    image_cache: std::collections::HashMap<String, egui::TextureHandle>,
    add_item_state: AddItemState,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            active_tab: Tab::Inventory,
            inventory_map: std::collections::HashMap::new(), // Initialized empty
            inventory_list: Vec::new(),                      // Initialized empty
            outfits_list: Vec::new(),                        // Initialized empty
            selected_inventory_item_id: None,
            outfit_collage_cache: HashMap::new(),
            show_create_outfit_popup: false,
            outfit_prompt: String::new(),
            outfit_state: OutfitState::Idle,
            selected_outfit_id: None,
            navigation_source_outfit_id: None,
            add_item_state: AddItemState::Idle { error: None },
            image_cache: std::collections::HashMap::new(),
        }
    }
}
impl MyApp {
    fn get_cached_image(&mut self, ctx: &egui::Context, path_str: &str) -> egui::TextureId {
        // 1. If it's already cached, return the ID instantly
        if let Some(handle) = self.image_cache.get(path_str) {
            return handle.id();
        }

        // 2. Try reading file bytes from disk
        let texture_handle = match std::fs::read(path_str) {
            Ok(file_bytes) => {
                match egui_extras::image::load_image_bytes(&file_bytes) {
                    Ok(color_image) => ctx.load_texture(path_str, color_image, Default::default()),
                    Err(e) => {
                        println!("❌ Failed to parse image bytes for '{}': {:?}", path_str, e);
                        self.fallback_placeholder(ctx)
                    }
                }
            }
            Err(e) => {
                // This will print out exactly what's wrong (e.g., "Permission Denied" or "File Not Found")
                println!("❌ Failed to read file path '{}': {:?}", path_str, e);
                
                // Also print where the app is looking right now!
                if let Ok(cwd) = std::env::current_dir() {
                    println!("   Current Working Directory is: {:?}", cwd);
                }
                
                self.fallback_placeholder(ctx)
            }
        };

        // 3. Insert into HashMap cache and return ID
        let id = texture_handle.id();
        self.image_cache.insert(path_str.to_string(), texture_handle);
        id
    }

    fn fallback_placeholder(&self, ctx: &egui::Context) -> egui::TextureHandle {
        // Use standard egui::ColorImage constructor (no struct fields, no bytemuck)
        let size = [400, 300];
        // Create a Vec containing every single pixel (400 * 300) set to gray
        let pixels = vec![egui::Color32::from_rgb(128, 128, 128); size[0] * size[1]];
    
        let color_image = egui::ColorImage::new(size, pixels);
        ctx.load_texture("fallback_placeholder", color_image, Default::default())
    }
}
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        // --- TOP BAR (Centered Tabs) ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // A trick to center the tab buttons: push flexible space on both sides
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true), |ui| {
                    ui.columns(3, |columns| {
                        columns[0].vertical_centered(|ui| {
                            ui.selectable_value(&mut self.active_tab, Tab::Inventory, "Inventory");
                        });
                        columns[1].vertical_centered(|ui| {
                            ui.selectable_value(&mut self.active_tab, Tab::Outfits, "Outfits");
                        });
                        columns[2].vertical_centered(|ui| {
                            ui.selectable_value(&mut self.active_tab, Tab::AddItem, "Add Item");
                        });
                    });
                });
            });
            ui.add_space(8.0);
        });

        // --- MAIN CONTENT AREA ---
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Inventory => self.show_inventory(ui),
                Tab::Outfits => self.show_outfits(ui),
                Tab::AddItem => self.show_add_item(ctx, ui),
            }
        });
        if let OutfitState::Processing { receiver } = &self.outfit_state {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(outfit) => {
                        self.outfits_list.push(outfit);
                        self.outfit_state = OutfitState::Idle;
                    },
                    Err(e) => println!("Error: {}", e),
                }
            }
        }
    }
}

// --- TAB IMPLEMENTATIONS ---
impl MyApp {
    // TAB 1: Inventory
    fn show_inventory(&mut self, ui: &mut egui::Ui) {
        if let Some(item_id) = self.selected_inventory_item_id.clone() {
            // --- DETAIL VIEW ---
            let mut go_back = false;
            let mut return_to_outfit = false;

            // Dynamic Back Button text based on where the user came from
            let back_text = if self.navigation_source_outfit_id.is_some() {
                "⬅ Back to Outfit"
            } else {
                "⬅ Back to Inventory"
            };

            if ui.button(back_text).clicked() {
                if self.navigation_source_outfit_id.is_some() {
                    return_to_outfit = true;
                } else {
                    go_back = true;
                }
            }
            ui.separator();
            // Step A: Extract only the image path we need to clear the borrow context
            let img_path = self.inventory_map.get(&item_id).map(|item| item.image_path.clone());

            // Step B: Now we can safely invoke the mutable image cache lookup
            let texture_id = if let Some(path) = &img_path {
                &self.get_cached_image(ui.ctx(), path)
            } else {
                &self.get_cached_image(ui.ctx(), "")
            };

            // Step C: Look up the item data *again* to draw the text properties safely
            if let Some(item) = &self.inventory_map.clone().get(&item_id) {
                ui.horizontal(|ui| {
                    ui.heading(format!("Item: {}", item.id));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let delete_btn = egui::Button::new(egui::RichText::new("🗑 Delete Item").color(egui::Color32::LIGHT_RED));
                            if ui.add(delete_btn).clicked() {
                                // Call our removal method using a cloned ID string to avoid borrow errors
                                self.delete_inventory_item(&item_id.clone());
                            }
                    });
                });
                // Draw real image
                let img_size = egui::vec2(400.0,300.0);
                if let Some(texture_handle) = self.image_cache.get(&item.image_path) {
                    let preview_image = egui::Image::new(texture_handle)
                        // 2. Set your bounding box
                        .max_size(egui::vec2(400.0, 300.0)) 
                        .fit_to_exact_size(img_size)
                        // 3. Add your styling
                        .corner_radius(8.0);
                    ui.add(preview_image);
                }
                ui.add_space(10.0);
                
                egui::Grid::new("item_properties").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                    ui.label("Category:"); ui.label(&item.category); ui.end_row();
                    ui.label("Style:"); ui.label(&item.style); ui.end_row();
                    ui.label("Season:"); ui.label(&item.season); ui.end_row();
                    ui.label("Occasion:"); ui.label(&item.occasion); ui.end_row();
                    ui.label("Colors:"); ui.label(format!("{}, {}", item.primary_color, item.secondary_color)); ui.end_row();
                    ui.label("Material:"); ui.label(&item.material); ui.end_row();
                    ui.label("Fit:"); ui.label(&item.fit); ui.end_row();
                });
            }

            if go_back {
                self.selected_inventory_item_id = None;
            }

            if return_to_outfit {
                // 1. Point our outfit manager right back to the original outfit
                self.selected_outfit_id = self.navigation_source_outfit_id.clone();
                // 2. Flip tabs back to outfits
                self.active_tab = Tab::Outfits;
                // 3. Reset the navigational state tracker
                self.selected_inventory_item_id = None;
                self.navigation_source_outfit_id = None;
            }
        } else {
            self.navigation_source_outfit_id = None;
            ui.heading("Inventory");
            // Pre-collect data so we don't hold a reference to 'self.inventory_list' during the loop
            let items_to_render: Vec<(String, String)> = self.inventory_list
                .iter()
                .map(|i| (i.category.clone(), i.image_path.clone()))
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (category, path) in items_to_render {
                        let texture_id = self.get_cached_image(ui.ctx(), &path);
                        
                        // 1. Allocate the click area
                        let card_size = egui::vec2(200.0, 240.0);
                        let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());
                        
                        // 2. Draw the background card
                        let bg_color = if response.hovered() { egui::Color32::from_gray(70) } else { egui::Color32::from_gray(40) };
                        ui.painter().rect_filled(rect, 6.0, bg_color);
                        
                        // 3. Create a sub-UI area strictly for the content inside this rectangle
                        ui.allocate_ui_at_rect(rect, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(5.0); // Padding from top
                                
                                // Draw the Image
                                //ui.add(egui::Image::from_texture((texture_id, egui::vec2(90.0, 85.0))).corner_radius(4.0));
                                let size = egui::vec2( 200.0, 200.0);
                                if let Some(texture_handle) = self.image_cache.get(&path) {
                                    let thumbnail = egui::Image::new(texture_handle)
                                        .max_size(size)
                                        .fit_to_exact_size(size)
                                        .corner_radius(4.0);
                                        
                                    ui.add(thumbnail);
                                }
                                ui.add_space(5.0); // Gap between image and text
                                
                                // Draw the Label
                                ui.label(egui::RichText::new(&category).size(11.0).color(egui::Color32::WHITE));
                            });
                        });

                        if response.clicked() {
                            if let Some(item) = self.inventory_list.iter().find(|i| i.image_path == path) {
                                self.selected_inventory_item_id = Some(item.id.clone());
                            }
                        }
                    }
                });
            });
        }
    }
    fn delete_inventory_item(&mut self, item_id: &str) {
        // 1. If it exists, find the file path and delete the image from disk
        if let Some(item) = self.inventory_map.remove(item_id) {
            let _ = std::fs::remove_file(&item.image_path);
            
            // Also clean up your UI image cache so egui forgets the texture handle
            self.image_cache.remove(&item.image_path);
        }

        // 2. Remove it from your runtime display vector
        self.inventory_list.retain(|i| i.id != item_id);

        // 3. Save the clean vector back to inventory.json
        if let Ok(updated_string) = serde_json::to_string_pretty(&self.inventory_list) {
            let _ = std::fs::write("inventory.json", updated_string);
        }
        
        // 4. Close the detail view if the deleted item was open
        if self.selected_inventory_item_id.as_deref() == Some(item_id) {
            self.selected_inventory_item_id = None;
        }
    }

    fn delete_outfit(&mut self, outfit_id: &str) {
        // 1. Find the outfit, delete its collage image, and drop from image cache
        if let Some(index) = self.outfits_list.iter().position(|o| o.id == outfit_id) {
            let outfit = &self.outfits_list[index];
            let _ = std::fs::remove_file(&outfit.collage_path);
            self.image_cache.remove(&outfit.collage_path);
        }

        // 2. Remove it from your runtime display vector
        self.outfits_list.retain(|o| o.id != outfit_id);

        // 3. Save the clean vector back to outfits.json
        if let Ok(updated_string) = serde_json::to_string_pretty(&self.outfits_list) {
            let _ = std::fs::write("outfits.json", updated_string);
        }

        // 4. Close the detail view if the deleted outfit was open
        if self.selected_outfit_id.as_deref() == Some(outfit_id) {
            self.selected_outfit_id = None;
        }
    }
    fn process_new_outfit_with_ai(prompt: String, items: Vec<InventoryItem>) -> std::sync::mpsc::Receiver<Result<Outfit, String>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let ollama = ollama_rs::Ollama::default();
                
                // 1. Construct prompt to pick IDs from provided items
                let item_list_json = serde_json::to_string(&items).unwrap();
                let full_prompt = format!(
                    "Create an outfit from these items: {}. Prompt: {}. \
                    Return JSON: {{ \"name\": \"string\", \"occasion\": \"string\", \"style\": \"string\", \
                    \"season\": [], \"weather\": {{}}, \"items\": [], \"score\": \"integer\", \"reasoning\": [] }} the json items array should be an array of the items ids only.", 
                    item_list_json, 
                    prompt
                );
                // 2. Call Ollama
                let fast_options = ollama_rs::models::ModelOptions::default()
                    .temperature(0.0)      // 0.0 makes the model deterministic and much faster
                    .num_predict(120)      // Caps the length so it stops instantly when JSON finishes
                    .top_k(20)             // Limits pool of choices for quicker token evaluation
                    .top_p(0.5);
                let result = ollama.generate(ollama_rs::generation::completion::request::GenerationRequest::new("gemma3:4b".into(), full_prompt)).await; //.options(fast_options)).await;
                
                // 3. Logic to create collage:
                // Use your create_outfit_collage function here to save a file to assets/collages/
                // ... logic to save file ...
                
                // 4. Send back the final Outfit struct
                // ... after receiving the AI response string ...
                
                let raw_response = match result {
                    Ok(res) => res.response, // This is already the string you want!
                    Err(e) => {
                        // Handle the error (send it to your channel so the UI sees it)
                        let _ = sender.send(Err(format!("Pipeline error: {}", e)));
                        return; // Exit the thread gracefully
                    }
                };

                // 3. You now have 'raw_response' as a standard Rust String
                println!("AI returned: {}", raw_response);
                let ai_response: String = raw_response;

                // Parse the raw string into a generic JSON value first
                let mut parsed_json: serde_json::Value = serde_json::from_str(&ai_response).unwrap_or_default();
                // 6. Clean up the response from any accidental markdown block wrappers
                let clean_json = &ai_response.trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_start_matches("```json")
                    .trim_end_matches("```")
                    .trim();

                // 7. Parse the cleaned string into our local structures
                let mut parsed_json: serde_json::Value = match serde_json::from_str(clean_json) {
                    Ok(v) => v,
                    Err(e) => { 
                        let _ = sender.send(Err(format!("JSON Parse Fail: {}. Raw output was: {}", e, ai_response))); 
                        return; 
                    }
                };
                // Inject your internal meta-data
                let id = format!("outfit_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
                parsed_json["id"] = serde_json::Value::String(id.clone());
                parsed_json["created_at"] = serde_json::Value::String("2026-06-17".to_string()); // Or use chrono crate
                parsed_json["collage_path"] = serde_json::Value::String(format!("assets/outfits/{}.png", id));
                //parsed_json["score"] = serde_json::json!(0);
                parsed_json["user_rating"] = serde_json::Value::Null;
                parsed_json["liked"] = serde_json::Value::Bool((false));
                parsed_json["favorite"] = serde_json::Value::Bool((false));
                parsed_json["times_worn"] = serde_json::json!(0);
                parsed_json["last_worn"] = serde_json::Value::Null;
                
                let chosen_ids: Vec<String> = parsed_json["items"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                // Convert the JSON value to your actual Outfit struct
                let final_outfit: Outfit = match serde_json::from_value(parsed_json) {
                    Ok(outfit) => outfit,
                    Err(e) => {
                        // Send the mapping error back to your UI thread
                        let _ = sender.send(Err(format!("Schema mapping failure: {}", e)));
                        return; // Gracefully stop execution in this background thread
                    }
                };
                if let Err(e) = std::fs::create_dir_all("assets/outfits") {
                    let _ = sender.send(Err(format!("Could not create collages directory: {}", e)));
                    return;
                }
                
                let item_paths: Vec<String> = items.iter()
                    .filter(|item| chosen_ids.contains(&item.id))
                    .take(4)
                    .map(|item| item.image_path.clone())
                    .collect();

                // 3. Call your custom function!
                let canvas = create_outfit_collage(&item_paths);

                // 4. Save the returned RgbaImage directly to the disk
                if let Err(e) = canvas.save(&final_outfit.collage_path) {
                    let _ = sender.send(Err(format!("Failed to save collage image: {}", e)));
                    return;
                }
                let mut current_items: Vec<Outfit> = std::fs::read_to_string("outfits.json")
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                current_items.push(final_outfit.clone());

                if let Ok(updated_string) = serde_json::to_string_pretty(&current_items) {
                    let _ = std::fs::write("outfits.json", updated_string);
                    println!("saving outfit!!");
                }

                // Send the ready item straight back to our main eframe loop
                
                let _ = sender.send(Ok(final_outfit));
                
            });
        });
        receiver
    }
    // TAB 2: Outfits
    fn show_outfits(&mut self, ui: &mut egui::Ui) {
        if self.show_create_outfit_popup {
            egui::Window::new("Create Outfit")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {

                    ui.label("Describe the outfit you want:");

                    ui.text_edit_multiline(
                        &mut self.outfit_prompt
                    );

                    if ui.button("Generate").clicked() {
                        // 1. Gather selected items (you might need to track selected items in MyApp)
                        let selected = self.inventory_list.clone(); // Replace with your actual selection logic
                        
                        // 2. Start Thread
                        let rx = Self::process_new_outfit_with_ai(self.outfit_prompt.clone(), selected);
                        self.outfit_state = OutfitState::Processing { receiver: rx };
                        self.show_create_outfit_popup = false;
                        println!("Generating outfit!!");
                    }

                    if ui.button("Cancel").clicked() {
                        self.show_create_outfit_popup = false;
                    }
                });
        }
        if let Some(outfit_id) = &self.selected_outfit_id {
            // --- OUTFIT DETAIL VIEW ---
            let mut go_back = false;
            let mut jump_to_inventory_id: Option<String> = None;

            if ui.button("⬅ Back to Outfits").clicked() {
                go_back = true;
            }
            ui.separator();

            // Step A: Find and Clone the outfit to completely free up the borrow on self
            let current_outfit = self.outfits_list.iter().find(|o| o.id == *outfit_id).cloned();

            if let Some(outfit) = current_outfit {

                let texture_id =
                    self.get_cached_image(ui.ctx(), &outfit.collage_path);
                
                ui.horizontal(|ui| {
                    ui.heading(&outfit.name);
                    if outfit.favorite { ui.colored_label(egui::Color32::from_rgb(255, 215, 0), "⭐ Favorite"); }
                    if outfit.liked { ui.colored_label(egui::Color32::LIGHT_RED, "❤️ Liked"); }
                    
                    // Push the delete button to the far right of the outfit header row
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let delete_btn = egui::Button::new(egui::RichText::new("🗑 Delete Outfit").color(egui::Color32::LIGHT_RED));
                        if ui.add(delete_btn).clicked() {
                            self.delete_outfit(&outfit.id.clone());
                        }
                    });
                });
                
                ui.small(format!("ID: {} | Created: {}", outfit.id, outfit.created_at));
                ui.add_space(10.0);

                // Draw the outfit image
                let img_size = egui::vec2(400.0,300.0);
                /* if let Some(texture_handle) = self.image_cache.get(&img_path) {
                        let preview_image = egui::Image::new(texture_handle)
                            // 2. Set your bounding box
                            .max_size(egui::vec2(400.0, 300.0)) 
                            .fit_to_exact_size(img_size)
                            // 3. Add your styling
                            .corner_radius(8.0);
                        ui.add(preview_image);
                    } */
                if let Some(texture_handle) =
                    self.image_cache.get(&outfit.collage_path)
                {
                    ui.add(
                        egui::Image::new(texture_handle)
                            .fit_to_exact_size(egui::vec2(400.0, 300.0))
                            .corner_radius(8.0)
                    );
                }

                ui.add_space(15.0);

                egui::Grid::new("outfit_metadata_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                    ui.label("Style:"); ui.label(&outfit.style); ui.end_row();
                    ui.label("Occasion:"); ui.label(&outfit.occasion); ui.end_row();
                    ui.label("Seasons:"); ui.label(outfit.season.join(", ")); ui.end_row();
                    
                    let rating_str = outfit.user_rating.map(|r| r.to_string()).unwrap_or_else(|| "Unrated".to_string());
                    ui.label("Score / Rating:"); ui.label(format!("Score: {} | User Rating: {}", outfit.score, rating_str)); ui.end_row();

                    let last_worn_str = outfit.last_worn.as_deref().unwrap_or("Never");
                    ui.label("Stats:"); ui.label(format!("Worn {} times (Last: {})", outfit.times_worn, last_worn_str)); ui.end_row();
                });

                if !outfit.reasoning.is_empty() {
                    ui.add_space(15.0);
                    ui.strong("AI Styling Reasoning:");
                    for line in &outfit.reasoning {
                        ui.label(format!("• {}", line));
                    }
                }

                ui.add_space(20.0);
                ui.heading("Included Inventory Items:");
                ui.horizontal_wrapped(|ui| {
                    // Collect data to break the borrow
                    let sub_items: Vec<(String, String, String)> = outfit.items.iter()
                        .filter_map(|id| self.inventory_map.get(id))
                        .map(|item| (item.id.clone(), item.category.clone(), item.image_path.clone()))
                        .collect();

                    for (id, category, path) in sub_items {
                        let sub_texture_id = self.get_cached_image(ui.ctx(), &path);
                        
                        // 1. Allocate the space for the card
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(90.0, 110.0), egui::Sense::click());
                        
                        // 2. Draw the background
                        let bg_color = if response.hovered() { egui::Color32::from_rgb(50, 70, 100) } else { egui::Color32::from_rgb(30, 45, 70) };
                        ui.painter().rect_filled(rect, 5.0, bg_color);
                        
                        // 3. Use allocate_ui_at_rect to perfectly center content inside the card
                        ui.allocate_ui_at_rect(rect, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(5.0);
                                
                                // Draw the image centered
                                ui.add(egui::Image::from_texture((sub_texture_id, egui::vec2(80.0, 75.0))).corner_radius(3.0));
                                
                                ui.add_space(5.0);
                                
                                // Draw the text centered
                                ui.label(egui::RichText::new(&category).size(10.0).color(egui::Color32::WHITE));
                            });
                        });

                        // 4. Handle click logic
                        if response.clicked() {
                            jump_to_inventory_id = Some(id); // Or use your logic to find the original ID
                        }
                    }
                });

                if go_back { self.selected_outfit_id = None; }
                if let Some(target_id) = jump_to_inventory_id {
                    // 1. Remember what outfit we are currently looking at!
                    self.navigation_source_outfit_id = self.selected_outfit_id.clone(); 
                    
                    // 2. Jump to the item
                    self.selected_inventory_item_id = Some(target_id);
                    self.active_tab = Tab::Inventory;
                }
            }
        } else {
            // --- OUTFIT GRID VIEW ---
            ui.heading("Outfits");
            if ui.button("➕ Create Outfit").clicked() {
                self.show_create_outfit_popup = true;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                let outfits_to_render: Vec<(String, String, String)> =
                    self.outfits_list
                        .iter()
                        .map(|o| (
                            o.id.clone(),
                            o.name.clone(),
                            o.collage_path.clone(),
                        ))
                        .collect();
                ui.horizontal_wrapped(|ui| {
                    for (id, name, collage_path) in outfits_to_render {

                        let texture_id =
                            self.get_cached_image(ui.ctx(), &collage_path);

                        let card_size = egui::vec2(220.0, 260.0);

                        let (rect, response) =
                            ui.allocate_exact_size(
                                card_size,
                                egui::Sense::click(),
                            );

                        let bg_color = if response.hovered() {
                            egui::Color32::from_gray(70)
                        } else {
                            egui::Color32::from_gray(40)
                        };

                        ui.painter()
                            .rect_filled(rect, 6.0, bg_color);

                        ui.allocate_ui_at_rect(rect, |ui| {
                            ui.vertical_centered(|ui| {

                                ui.add_space(5.0);

                                if let Some(texture_handle) =
                                    self.image_cache.get(&collage_path)
                                {
                                    let thumbnail =
                                        egui::Image::new(texture_handle)
                                            .fit_to_exact_size(
                                                egui::vec2(210.0, 210.0)
                                            )
                                            .corner_radius(4.0);

                                    ui.add(thumbnail);
                                }

                                ui.add_space(5.0);

                                ui.label(
                                    egui::RichText::new(&name)
                                        .size(11.0)
                                        .color(egui::Color32::WHITE)
                                );
                            });
                        });

                        if response.clicked() {
                            self.selected_outfit_id = Some(id);
                        }
                    }
                });
            });
        }
        
    }
    fn process_new_item_with_ai(original_path_str: String) -> std::sync::mpsc::Receiver<Result<InventoryItem, String>> {
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            // Create a basic tokio runtime to run the async ollama commands inside our thread
            let rt = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(e) => { let _ = sender.send(Err(e.to_string())); return; }
            };

            rt.block_on(async {
                let src_path = std::path::Path::new(&original_path_str);
                
                // 1. Generate Unique ID and copy the file locally
                let item_id = format!("item_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
                let extension = src_path.extension().and_then(|os| os.to_str()).unwrap_or("png");
                let local_image_path = format!("assets/items/{}.{}", item_id, extension);
                
                if std::fs::create_dir_all("assets").is_err() || std::fs::copy(&src_path, &local_image_path).is_err() {
                    let _ = sender.send(Err("Failed to save copy to assets folder".to_string()));
                    return;
                }

                // 2. Initialize the local Ollama client (defaults to localhost:11434)
                let ollama = ollama_rs::Ollama::default();

                // 3. Build the strict prompt
                let prompt = "Analyze this clothing item image. Output exactly a valid raw JSON object matching this schema structure. Do not wrap it inside a markdown block:
{
  \"category\": \"Shirt/Pants/Jacket/Shoes/etc, can choose others than these\",
  \"primary_color\": \"dominant color\",
  \"secondary_color\": \"accent color or None\",
  \"style\": \"Casual/Formal/Sporty/etc, can choose others than these\",
  \"season\": \"Summer/Winter/Spring/Autumn\",
  \"occasion\": \"Everyday/Work/Party/etc, can choose others than these\",
  \"material\": \"Cotton/Denim/Leather/etc\",
  \"fit\": \"Regular/Slim/Relaxed/Baggy\"
}".to_string();

                // 4. Create the generation request with the image attached via its path
                let file_bytes = match std::fs::read(&local_image_path) {
                    Ok(bytes) => bytes,
                    Err(e) => { let _ = sender.send(Err(format!("Failed to read cloned file bytes: {}", e))); return; }
                };
                use base64::Engine;
                let b64_str = base64::engine::general_purpose::STANDARD.encode(file_bytes);
                let ollama_image = ollama_rs::generation::images::Image::from_base64(b64_str);
                let fast_options = ollama_rs::models::ModelOptions::default()
                    .temperature(0.0)      // 0.0 makes the model deterministic and much faster
                    .num_predict(120)      // Caps the length so it stops instantly when JSON finishes
                    .top_k(20)             // Limits pool of choices for quicker token evaluation
                    .top_p(0.5);
                let request = ollama_rs::generation::completion::request::GenerationRequest::new(
                    "gemma3:4b".to_string(),
                    prompt,
                ).options(fast_options)
                .add_image(ollama_image); // ollama-rs handles the base64 translation automatically here!

                // 5. Run the model request
                let response = match ollama.generate(request).await {
                    Ok(res) => res.response,
                    Err(e) => { let _ = sender.send(Err(format!("Ollama error: {}", e))); return; }
                };

                // 6. Clean up the response from any accidental markdown block wrappers
                let clean_json = response.trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();

                // 7. Parse the cleaned string into our local structures
                let mut parsed_data: serde_json::Value = match serde_json::from_str(clean_json) {
                    Ok(v) => v,
                    Err(e) => { 
                        let _ = sender.send(Err(format!("JSON Parse Fail: {}. Raw output was: {}", e, response))); 
                        return; 
                    }
                };

                // Inject our systemic application structural meta values
                parsed_data["id"] = serde_json::Value::String(item_id.clone());
                parsed_data["image_path"] = serde_json::Value::String(local_image_path);

                let final_item: InventoryItem = match serde_json::from_value(parsed_data) {
                    Ok(item) => item,
                    Err(e) => { let _ = sender.send(Err(format!("Schema mapping failure: {}", e))); return; }
                };

                // 8. Save directly to the local JSON database file
                let mut current_items: Vec<InventoryItem> = std::fs::read_to_string("inventory.json")
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                current_items.push(final_item.clone());

                if let Ok(updated_string) = serde_json::to_string_pretty(&current_items) {
                    let _ = std::fs::write("inventory.json", updated_string);
                }

                // Send the ready item straight back to our main eframe loop
                let _ = sender.send(Ok(final_item));
            });
        });

        receiver
    }
    // TAB 3: Add Item
    fn show_add_item(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Add Item");
            ui.add_space(20.0);

            // 1. Create a temporary variable to hold any upcoming state changes
            let mut next_state: Option<AddItemState> = None;

            // 2. Match on the current state (borrowing it)
            match &self.add_item_state {
                AddItemState::Idle { error} => {
                    if let Some(err_msg) = error {
                        ui.add_space(10.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(70, 20, 20)) // Dark red warning box
                            .stroke(egui::Stroke::new(1.0, egui::Color32::LIGHT_RED))
                            .corner_radius(6.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⚠️ Pipeline Error:").strong().color(egui::Color32::LIGHT_RED));
                                    ui.label(egui::RichText::new(err_msg).color(egui::Color32::WHITE));
                                });
                            });
                        ui.add_space(15.0);
                    }
                    if ui.button("Select Image from OS").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("image", &["png", "jpg", "jpeg", "webp"])
                            .pick_file()
                        {
                            // Queue the state change instead of applying it immediately
                            next_state = Some(AddItemState::Loading {
                                start_time: Instant::now(),
                                file_path: path.display().to_string(),
                            });
                        }
                    }
                }
                AddItemState::Loading { start_time, file_path } => {
                    ui.spinner();
                    ui.label("Preparing file path selection...");
                    ctx.request_repaint();

                    // Instead of doing the fs::read here, we trigger the background thread immediately!
                    if start_time.elapsed().as_millis() >= 100 {
                        // Start up the background thread processing chain using the path picked by the user
                        let rx = Self::process_new_item_with_ai(file_path.clone());
                        next_state = Some(AddItemState::ProcessingAI { receiver: rx });
                    }
                }
                AddItemState::ProcessingAI { receiver } => {
                    ui.spinner();
                    ui.colored_label(egui::Color32::LIGHT_BLUE, "🤖 Ollama Gemma3:4b is analyzing garment properties...");
                    ctx.request_repaint();

                    if let Ok(result) = receiver.try_recv() {
                        match result {
                            Ok(new_item) => {
                                if let Ok(file_bytes) = std::fs::read(&new_item.image_path) {
                                    if let Ok(color_image) = egui_extras::image::load_image_bytes(&file_bytes) {
                                        let texture = ctx.load_texture(&new_item.image_path, color_image, Default::default());
                                        
                                        self.inventory_list.push(new_item.clone());
                                        self.inventory_map.insert(new_item.id.clone(), new_item.clone());

                                        next_state = Some(AddItemState::ViewingDetails {
                                            file_path: new_item.image_path.clone(),
                                            texture,
                                        });
                                    }
                                }
                            }
                        Err(err_msg) => {
                                println!("❌ AI pipeline broke: {}", err_msg);
                            next_state = Some(AddItemState::Idle { error: Some(err_msg) });
                        }
                        }
                    }
                }
                // Inside Tab::AddItem -> AddItemState::ViewingDetails arm:
                AddItemState::ViewingDetails { file_path, texture } => {
                    if ui.button("⬅ Cancel / Add Another").clicked() {
                        next_state = Some(AddItemState::Idle { error: None });
                    }
                    ui.separator();
                    ui.heading("New Item Processed!");

                    // --- DISPLAY THE USER'S UPLOADED IMAGE ---
                    ui.add(
                        egui::Image::new(texture)
                            .max_width(400.0)
                            .rounding(8.0)
                    );

                    ui.label(format!("File loaded from: {}", file_path));
                }
            }

            // 3. Apply the state change after the match block is over and the borrow has ended
            if let Some(state) = next_state {
                self.add_item_state = state;
            }
        });
    }

}

fn create_outfit_collage(image_paths: &[String]) -> RgbaImage {
    // 1. Create a blank canvas (e.g., 400x400)
    let mut canvas = RgbaImage::new(400, 400);

    // 2. Load and draw each item (simplified example)
    for (i, path) in image_paths.iter().enumerate() {
        if let Ok(img) = image::open(path) {
            // Resize image to fit a section of the canvas
            let scaled = img.resize(200, 200, image::imageops::FilterType::Lanczos3);
            
            // Calculate position (e.g., a simple 2x2 grid)
            let x = (i % 2 * 200) as u32;
            let y = (i / 2 * 200) as u32;
            
            // Paste the item onto the canvas
            canvas.copy_from(&scaled, x, y).unwrap();
        }
    }
    canvas
}

fn ensure_json_files_exist() {
    // 1. Ensure inventory.json exists
    if !Path::new("inventory.json").exists() {
        let default_inventory = vec![
            InventoryItem {
                id: "item_001".to_string(),
                category: "Shirt".to_string(),
                primary_color: "White".to_string(),
                secondary_color: "None".to_string(),
                style: "Casual".to_string(),
                season: "Summer".to_string(),
                occasion: "Everyday".to_string(),
                material: "Cotton".to_string(),
                fit: "Regular".to_string(),
                image_path: "assets/items/sample_shirt.png".to_string(),
            }
        ];
        
        // Serialize the sample data into clean, readable JSON format
        if let Ok(json_string) = serde_json::to_string_pretty(&default_inventory) {
            let _ = fs::write("inventory.json", json_string);
            println!("Created a fresh inventory.json with sample data!");
        }
    }

    // 2. Ensure outfits.json exists
    if !Path::new("outfits.json").exists() {
        // Build a sample outfit using your exact new JSON schema structure
        let default_outfits = vec![
            serde_json::json!({
                "id": "outfit_001",
                "name": "Summer Casual White",
                "occasion": "Everyday",
                "style": "Casual",
                "season": ["Summer"],
                "weather": { "temperature": "hot", "condition": "sunny" },
                "items": ["item_001"], // Links directly to the sample item ID above
                "score": 85,
                "user_rating": serde_json::Value::Null,
                "liked": false,
                "favorite": false,
                "times_worn": 0,
                "last_worn": serde_json::Value::Null,
                "reasoning": ["Classic light setup for warm weather."],
                "created_at": "2026-06-16"
            })
        ];

        if let Ok(json_string) = serde_json::to_string_pretty(&default_outfits) {
            let _ = fs::write("outfits.json", json_string);
            println!("Created a fresh outfits.json with sample data!");
        }
    }
}
// 4. Main Function
fn main() -> eframe::Result<()> {
    ensure_json_files_exist();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Inventory & Outfits Manager",
        options,
        Box::new(|_cc| {
            let inventory_json = std::fs::read_to_string("inventory.json").unwrap_or_else(|_| "[]".to_string());

            // 2. Parse the JSON into the ordered list vector
            let inventory_list: Vec<InventoryItem> = serde_json::from_str(&inventory_json).unwrap_or_default();

            // 3. Build the lookup map using the parsed list data
            let mut inventory_map = std::collections::HashMap::new();
            for item in &inventory_list {
                inventory_map.insert(item.id.clone(), item.clone());
            }

            let outfits_json = std::fs::read_to_string("outfits.json").unwrap_or_else(|_| "[]".to_string());
            let outfits_list: Vec<Outfit> = serde_json::from_str(&outfits_json).unwrap_or_default();

            // 2. Build the app state with the parsed data
            let app = MyApp {
                active_tab: Tab::Inventory,
                inventory_map,
                inventory_list,
                outfits_list,
                selected_inventory_item_id: None,
                selected_outfit_id: None,
                show_create_outfit_popup: false,
                outfit_prompt: String::new(),
                outfit_state: OutfitState::Idle,
                outfit_collage_cache: HashMap::new(),
                navigation_source_outfit_id: None,
                add_item_state: AddItemState::Idle { error: None },
                image_cache: std::collections::HashMap::new(),
            };

            Ok(Box::new(app))
        }),
    )
}