use eframe::egui;
use std::time::Instant;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    Idle,
    Loading { start_time: Instant, file_path: String },
    ViewingDetails { 
        file_path: String, 
        texture: egui::TextureHandle // Store the loaded texture here
    },
}

// 3. Main App Struct
struct MyApp {
    active_tab: Tab,
    
    // Loaded Collections
    inventory_map: HashMap<String, InventoryItem>, // Quick lookup by ID
    inventory_list: Vec<InventoryItem>,            // Ordered list for the grid
    outfits_list: Vec<Outfit>,
    
    // UI Selection Tracking (Now tracking by string IDs)
    selected_inventory_item_id: Option<String>,
    selected_outfit_id: Option<String>,
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
            selected_outfit_id: None,
            add_item_state: AddItemState::Idle,
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
    }
}

// --- TAB IMPLEMENTATIONS ---
impl MyApp {
    // TAB 1: Inventory
    fn show_inventory(&mut self, ui: &mut egui::Ui) {
        if let Some(item_id) = self.selected_inventory_item_id.clone() {
        // --- DETAIL VIEW ---
        let mut go_back = false;
        if ui.button("⬅ Back to Inventory").clicked() {
            go_back = true;
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
        if let Some(item) = self.inventory_map.get(&item_id) {
            ui.heading(format!("Item: {}", item.id));
            
            // Draw real image
            ui.add(egui::Image::from_texture((*texture_id, egui::vec2(400.0, 300.0))).corner_radius(8.0));
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
        } else {
            // --- GRID VIEW ---
            ui.heading("Inventory");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // Ensure this points to self.inventory_list!
                    for item in &self.inventory_list {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(100.0, 120.0), egui::Sense::click());
                        let color = if response.hovered() { egui::Color32::GRAY } else { egui::Color32::DARK_GRAY };
                        ui.painter().rect_filled(rect, 5.0, color);
                        
                        ui.painter().text(
                            rect.center_bottom() - egui::vec2(0.0, 12.0), 
                            egui::Align2::CENTER_BOTTOM, 
                            &item.category, 
                            egui::FontId::proportional(12.0), 
                            egui::Color32::WHITE
                        );

                        if response.clicked() {
                            self.selected_inventory_item_id = Some(item.id.clone());
                        }
                    }
                });
            });
        }
    }

    // TAB 2: Outfits
    fn show_outfits(&mut self, ui: &mut egui::Ui) {
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
            // Step B: Get the image path of the first item belonging to this outfit
            let mut img_path = String::new();
            if let Some(first_item_id) = outfit.items.first() {
                if let Some(item) = self.inventory_map.get(first_item_id) {
                    img_path = item.image_path.clone();
                }
            }

            // Step C: Secure the texture ID safely
            let texture_id = self.get_cached_image(ui.ctx(), &img_path);

            // Step D: Render the UI without any closure or borrow errors!
            ui.horizontal(|ui| {
                ui.heading(&outfit.name);
                if outfit.favorite { ui.colored_label(egui::Color32::from_rgb(255, 215, 0), "⭐ Favorite"); }
                if outfit.liked { ui.colored_label(egui::Color32::LIGHT_RED, "❤️ Liked"); }
            });
            
            ui.small(format!("ID: {} | Created: {}", outfit.id, outfit.created_at));
            ui.add_space(10.0);

            // Draw the outfit image
            ui.add(egui::Image::from_texture((texture_id, egui::vec2(400.0, 300.0))).corner_radius(8.0));
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
                for item_id in &outfit.items {
                    if let Some(inventory_item) = self.inventory_map.get(item_id) {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(90.0, 110.0), egui::Sense::click());
                        let color = if response.hovered() { egui::Color32::LIGHT_BLUE } else { egui::Color32::BLUE };
                        ui.painter().rect_filled(rect, 5.0, color);
                        
                        ui.painter().text(rect.center_bottom() - egui::vec2(0.0, 10.0), egui::Align2::CENTER_BOTTOM, &inventory_item.category, egui::FontId::proportional(11.0), egui::Color32::WHITE);

                        if response.clicked() {
                            jump_to_inventory_id = Some(inventory_item.id.clone());
                        }
                    } else {
                        ui.colored_label(egui::Color32::RED, format!("Missing Item [{}]", item_id));
                    }
                }
            });
        }

        if go_back { self.selected_outfit_id = None; }
        if let Some(target_id) = jump_to_inventory_id {
            self.selected_inventory_item_id = Some(target_id);
            self.active_tab = Tab::Inventory;
        }
        } else {
            // --- OUTFIT GRID VIEW ---
            ui.heading("Outfits");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for outfit in &self.outfits_list {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(130.0, 130.0), egui::Sense::click());
                        let color = if response.hovered() { egui::Color32::DARK_GREEN } else { egui::Color32::from_rgb(34, 112, 63) };
                        ui.painter().rect_filled(rect, 8.0, color);
                        
                        // Display the Outfit's text Name instead of ID inside the grid cards
                        let display_name = if outfit.name.is_empty() { &outfit.id } else { &outfit.name };
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, display_name, egui::FontId::proportional(14.0), egui::Color32::WHITE);

                        if response.clicked() {
                            self.selected_outfit_id = Some(outfit.id.clone());
                        }
                    }
                });
            });
        }
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
                AddItemState::Idle => {
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
                    ui.label("Processing image...");
                    ctx.request_repaint();

                    // Inside Tab::AddItem -> AddItemState::Loading arm:
                    if start_time.elapsed().as_secs() >= 2 {
                        // 1. Read the raw file bytes from the OS path
                        if let Ok(file_bytes) = std::fs::read(&file_path) {
                            
                            // 2. Convert those bytes into an egui-compatible ColorImage
                            if let Ok(color_image) = egui_extras::image::load_image_bytes(&file_bytes) {
                                
                                // 3. Upload the image to the GPU graphics context
                                let texture = ctx.load_texture(
                                    "user_uploaded_item",
                                    color_image,
                                    Default::default()
                                );

                                // 4. Move to the details view and hand over the texture
                                next_state = Some(AddItemState::ViewingDetails {
                                    file_path: file_path.clone(),
                                    texture, 
                                });
                            }
                        }
                    }
                }
                // Inside Tab::AddItem -> AddItemState::ViewingDetails arm:
                AddItemState::ViewingDetails { file_path, texture } => {
                    if ui.button("⬅ Cancel / Add Another").clicked() {
                        next_state = Some(AddItemState::Idle);
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
                image_path: "assets/sample_shirt.png".to_string(),
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
                add_item_state: AddItemState::Idle,
                image_cache: std::collections::HashMap::new(),
            };

            Ok(Box::new(app))
        }),
    )
}