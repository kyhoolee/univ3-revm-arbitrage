use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use alloy::providers::{Provider, ProviderBuilder, RootProvider}; // Import RootProvider
use alloy::transports::http::{Http, Client};
use alloy::network::Ethereum; // Vẫn cần Ethereum nếu Provider yêu cầu Network generic

// Định nghĩa kiểu cố định mà `on_http` trả về
// Đây là kiểu cụ thể, Sized, và implement Provider
type ConcreteHttpProvider = RootProvider<Http<Client>>;

pub struct MultiProvider {
    // Lưu trữ các Arc của kiểu cụ thể này
    providers: Vec<Arc<ConcreteHttpProvider>>,
    urls: Vec<String>,  // NEW: lưu song song URL string
    counter: AtomicUsize,
}

impl MultiProvider {
    // define len 
    pub fn len(&self) -> usize {
        self.providers.len()
    }


    pub fn new(urls: &[String]) -> Self {
        let providers = urls.iter()
            .map(|url| {
                let provider = ProviderBuilder::new()
                    .on_http(url.parse().expect("Invalid RPC URL"));
                // provider ở đây đã là ConcreteHttpProvider, không cần ép kiểu 'as dyn'
                Arc::new(provider)
            })
            .collect::<Vec<_>>();

        MultiProvider {
            providers,
            urls: urls.to_vec(), 
            counter: AtomicUsize::new(0),
        }
    }

    pub fn next(&self) -> (Arc<ConcreteHttpProvider>, String) {  // CHANGED: trả về tuple
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % self.providers.len();
        (self.providers[index].clone(), self.urls[index].clone())  // trả về luôn URL
    }
}

