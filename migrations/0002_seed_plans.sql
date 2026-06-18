INSERT INTO plans (id, code, name, price_per_machine_year, onboarding_fee, currency, features, is_active) VALUES
 (UUID(), 'basic',   'Passport Basic',   150000,  60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', false, 'predict', false), TRUE),
 (UUID(), 'live',    'Passport Live',    360000,  60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', true,  'predict', false), TRUE),
 (UUID(), 'predict', 'Passport Predict', 0,       60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', true,  'predict', true),  FALSE);
-- prices in paise: 150000 = ₹1,500/yr ; 360000 = ₹3,600/yr ; onboarding 60000 = ₹600.
-- Predict price 0 + is_active FALSE = roadmap placeholder.
