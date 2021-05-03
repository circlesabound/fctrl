// Schemas ported from src/schema.rs

export interface ResponseWithId {
    operation_id: string;
    status: OperationStatus;
    content: any;
}

export enum OperationStatus {
    Ack = 'Ack',
    Ongoing = 'Ongoing',
    Completed = 'Completed',
    Failed = 'Failed',
}
