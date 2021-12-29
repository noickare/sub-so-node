#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_support::{
        sp_runtime::traits::Hash,
        traits::{ Randomness, Currency, tokens::ExistenceRequirement },
        transactional
    };
    use sp_io::hashing::blake2_128;
    use scale_info::TypeInfo;

    #[cfg(feature = "std")]
    use frame_support::serde::{Deserialize, Serialize};

    type AccountOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // Struct for holding Kitty information.
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct ClueNFT<T: Config> {
        pub asset_url: [u8; 16],
        pub price: Option<BalanceOf<T>>,
        pub asset_type: AssetType,
        pub owner: AccountOf<T>,
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub enum AssetType {
        Video,
        Picture,
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types it depends on.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Currency: Currency<Self::AccountId>;

        type NFTRandomness: Randomness<Self::Hash, Self::BlockNumber>;

        #[pallet::constant]
        type MaxNFTOwned: Get<u32>;
    }

    // Errors.
    #[pallet::error]
    pub enum Error<T> {
        // TODO Part III
        /// Handles arithmetic overflow when incrementing the NFT counter.
        NFTCntOverflow,
        /// An account cannot own more NFTs than `MaxKittyCount`.
        ExceedMaxNFTOwned,
        /// Buyer cannot be the owner.
        BuyerIsNFTOwner,
        /// Cannot transfer a nft to its owner.
        TransferToSelf,
        /// Handles checking whether the nft exists.
        NFTNotExist,
        /// Handles checking that the Nft is owned by the account transferring, buying or setting a price for it.
        NotNFTOwner,
        /// Ensures the Nft is for sale.
        NFTNotForSale,
        /// Ensures that the buying price is greater than the asking price.
        NFTBidPriceTooLow,
        /// Ensures that an account has enough funds to purchase a Nft.
        NotEnoughBalance,
    }

    // Events.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new NFT was sucessfully created. \[sender, nft_id\]
        Created(T::AccountId, T::Hash),
        /// NFT price was sucessfully set. \[sender, nft_id, new_price\]
        PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
        /// A NFT was sucessfully transferred. \[from, to, nft_id\]
        Transferred(T::AccountId, T::AccountId, T::Hash),
        /// A NFT was sucessfully bought. \[buyer, seller, nft_id, bid_price\]
        Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
    }

    #[pallet::storage]
    #[pallet::getter(fn nft_cnt)]
    pub(super) type NFTCnt<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nft)]
    pub(super) type ClueNFTs<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::Hash,
        ClueNFT<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn nft_owned)]
    /// Keeps track of what accounts own what NFT.
    pub(super) type NFTOwned<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        BoundedVec<T::Hash, T::MaxNFTOwned>,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub clue_nfts: Vec<(T::AccountId, [u8; 16], AssetType)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> GenesisConfig<T> {
            GenesisConfig { clue_nfts: vec![] }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            // When building a nft from genesis config, we require the dna and gender to be supplied.
            for (acct, asset_url, asset_type) in &self.clue_nfts {
                let _ = <Pallet<T>>::mint(acct, asset_url.clone(), Some(asset_type.clone()));
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.




    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new unique NFT.
        ///
        /// The actual nft creation is done in the `mint()` function.
        #[pallet::weight(100)]
        pub fn create_nft(origin: OriginFor<T>, asset_url: [u8; 16], asset_type: AssetType) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let nft_id = Self::mint(&sender, asset_url, Some(asset_type.clone()))?;
            // Logging to the console
            log::info!("A NFT is born with ID: {:?}.", nft_id);

            Self::deposit_event(Event::Created(sender, nft_id));

            Ok(())
        }

        #[pallet::weight(100)]
        pub fn set_price(
            origin: OriginFor<T>,
            nft_id: T::Hash,
            new_price: Option<BalanceOf<T>>
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            ensure!(Self::is_nft_owner(&nft_id, &sender)?, <Error<T>>::NotNFTOwner);

            let mut nft = Self::nft(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;

            nft.price = new_price.clone();
            <ClueNFTs<T>>::insert(&nft_id, nft);

            Self::deposit_event(Event::PriceSet(sender, nft_id, new_price));

            Ok(())
        }

        #[pallet::weight(100)]
        pub fn transfer(
            origin: OriginFor<T>,
            to: T::AccountId,
            nft_id: T::Hash,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;

            ensure!(Self::is_nft_owner(&nft_id, &from)?, <Error<T>>::NotNFTOwner);

            ensure!(from != to, <Error<T>>::TransferToSelf);

            let to_owned = <NFTOwned<T>>::get(&to);
            ensure!((to_owned.len() as u32) < T::MaxNFTOwned::get(), <Error<T>>::ExceedMaxNFTOwned);

            Self::transfer_nft_to(&nft_id, &to)?;

            Self::deposit_event(Event::Transferred(from, to, nft_id));

            Ok(())
        }


        #[transactional]
        #[pallet::weight(100)]
        pub fn buy_nft(
            origin: OriginFor<T>,
            nft_id: T::Hash,
            bid_price: BalanceOf<T>
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;

            let nft = Self::nft(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;
            ensure!(nft.owner != buyer, <Error<T>>::BuyerIsNFTOwner);

            if let Some(ask_price) = nft.price {
                ensure!(ask_price <= bid_price, <Error<T>>::NFTBidPriceTooLow);
            } else {
                Err(<Error<T>>::NFTNotForSale)?;
            }

            ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

            let to_owned = <NFTOwned<T>>::get(&buyer);
            ensure!((to_owned.len() as u32) < T::MaxNFTOwned::get(), <Error<T>>::ExceedMaxNFTOwned);

            let to_owned = <NFTOwned<T>>::get(&buyer);
            ensure!((to_owned.len() as u32) < T::MaxNFTOwned::get(), <Error<T>>::ExceedMaxNFTOwned);

            let seller = nft.owner.clone();

            T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

            Self::transfer_nft_to(&nft_id, &buyer)?;

            Self::deposit_event(Event::Bought(buyer, seller, nft_id, bid_price));


            Ok(())
        }

        #[pallet::weight(100)]
        pub fn breed_nft(
            origin: OriginFor<T>,
            parent1: T::Hash,
            parent2: T::Hash,
            asset_url: [u8; 16],
            asset_type: AssetType,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            // Check: Verify `sender` owns both nfts (and both nfts exist).
            ensure!(Self::is_nft_owner(&parent1, &sender)?, <Error<T>>::NotNFTOwner);
            ensure!(Self::is_nft_owner(&parent2, &sender)?, <Error<T>>::NotNFTOwner);

            Self::mint(&sender, asset_url, Some(asset_type.clone()))?;

            Ok(())
        }
    }

    //** Our helper functions.**//

    impl<T: Config> Pallet<T> {

        // ACTION #4: helper function for NFT struct
        // fn gen_gender() -> Gender {
        //     let random = T::NFTRandomness::random(&b"gender"[..]).0;
        //     match random.as_ref()[0] % 2 {
        //         0 => Gender::Male,
        //         _ => Gender::Female,
        //     }
        // }


        // fn gen_dna() -> [u8; 16] {
        //     let payload = (
        //         T::NFTRandomness::random(&b"dna"[..]).0,
        //         <frame_system::Pallet<T>>::block_number(),
        //     );
        //     payload.using_encoded(blake2_128)
        //
        // }

        // Create new DNA with existing DNA
        // pub fn breed_dna(parent1: &T::Hash, parent2: &T::Hash) -> Result<[u8; 16], Error<T>> {
        //     let dna1 = Self::nft(parent1).ok_or(<Error<T>>::NFTNotExist)?.dna;
        //     let dna2 = Self::nft(parent2).ok_or(<Error<T>>::NFTNotExist)?.dna;
        //
        //     let mut new_dna = Self::gen_dna();
        //     for i in 0..new_dna.len() {
        //         new_dna[i] = (new_dna[i] & dna1[i]) | (!new_dna[i] & dna2[i]);
        //     }
        //     Ok(new_dna)
        // }

        // Helper to mint a NFT.
        pub fn mint(
            owner: &T::AccountId,
            asset_url: [u8; 16],
            asset_type: Option<AssetType>,
        ) -> Result<T::Hash, Error<T>> {
            let nft = ClueNFT::<T> {
                asset_url: asset_url.clone(),
                price: None,
                owner: owner.clone(),
                asset_type: asset_type.unwrap(),
            };

            let nft_id = T::Hashing::hash_of(&nft);

            // Performs this operation first as it may fail
            let new_cnt = Self::nft_cnt().checked_add(1)
                .ok_or(<Error<T>>::NFTCntOverflow)?;

            // Performs this operation first because as it may fail
            <NFTOwned<T>>::try_mutate(&owner, |nft_vec| {
                nft_vec.try_push(nft_id)
            }).map_err(|_| <Error<T>>::ExceedMaxNFTOwned)?;

            <ClueNFTs<T>>::insert(nft_id, nft);
            <NFTCnt<T>>::put(new_cnt);
            Ok(nft_id)
        }

        // Helper to check correct NFT owner
        pub fn is_nft_owner(nft_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
            match Self::nft(nft_id) {
                Some(nft) => Ok(nft.owner == *acct),
                None => Err(<Error<T>>::NFTNotExist)
            }
        }

        #[transactional]
        pub fn transfer_nft_to(
            nft_id: &T::Hash,
            to: &T::AccountId,
        ) -> Result<(), Error<T>> {
            let mut nft = Self::nft(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;

            let prev_owner = nft.owner.clone();

            <NFTOwned<T>>::try_mutate(&prev_owner, |owned| {
                if let Some(ind) = owned.iter().position(|&id| id == *nft_id) {
                    owned.swap_remove(ind);
                    return Ok(());
                }
                Err(())
            }).map_err(|_| <Error<T>>::NFTNotExist)?;

            nft.owner = to.clone();
            // by the current owner.
            nft.price = None;

            <ClueNFTs<T>>::insert(nft_id, nft);

            <NFTOwned<T>>::try_mutate(to, |vec| {
                vec.try_push(*nft_id)
            }).map_err(|_| <Error<T>>::ExceedMaxNFTOwned)?;

            Ok(())
        }
    }
}
